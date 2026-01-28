use hyper::{Body, Response, StatusCode};

use crate::{config, git, hammerspoon, projects, util::debug};

/// Return JSON with current and available projects (including tasks)
pub fn list_projects() -> Response<Body> {
    let tasks = projects::tasks();

    // Get currently open projects, using task info where available
    let mut current: Vec<_> = projects::lock()
        .open()
        .into_iter()
        .map(|p| {
            let project = tasks.get(&p.name).unwrap_or(&p);
            let mut obj = serde_json::json!({ "name": project.name });
            if let Some(home) = &project.home_project {
                obj["home_project"] = serde_json::json!(home);
                obj["path"] = serde_json::json!(project.path);
                if let Some(branch) = git::current_branch(&project.path) {
                    obj["branch"] = serde_json::json!(branch);
                }
            }
            obj
        })
        .collect();

    // Sort: projects without home_project first (alphabetically), then tasks (by home, name)
    current.sort_by(|a, b| {
        let a_home = a.get("home_project").and_then(|h| h.as_str());
        let b_home = b.get("home_project").and_then(|h| h.as_str());
        let a_name = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let b_name = b.get("name").and_then(|n| n.as_str()).unwrap_or("");

        match (a_home, b_home) {
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, None) => a_name.cmp(b_name),
            (Some(ah), Some(bh)) => (ah, a_name).cmp(&(bh, b_name)),
        }
    });

    let available = config::available_projects();
    let available: Vec<&str> = available.keys().map(|s| s.as_str()).collect();

    let json = serde_json::json!({
        "current": current,
        "available": available
    });

    Response::builder()
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string_pretty(&json).unwrap()))
        .unwrap()
}

pub fn debug_projects() -> Response<Body> {
    let projects = projects::lock();

    let output: Vec<serde_json::Value> = projects
        .all()
        .iter()
        .enumerate()
        .map(|(i, project)| {
            serde_json::json!({
                "index": i,
                "name": project.name,
                "path": project.path.display().to_string(),
                "aliases": project.aliases,
                "home_project": project.home_project,
            })
        })
        .collect();

    Response::builder()
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string_pretty(&output).unwrap()))
        .unwrap()
}

pub fn remove_project(name: &str) -> Response<Body> {
    let mut projects = projects::lock();
    if let Some(p) = projects.by_name(name) {
        config::TERMINAL.close(&p);
    }
    if projects.remove(name) {
        projects.print();
        Response::new(Body::from(format!("removed project: {}", name)))
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", name)))
            .unwrap()
    }
}

pub fn close_project(name: &str) {
    let mut projects = projects::lock();
    if let Some(p) = projects.resolve(name) {
        config::TERMINAL.close(&p);
        config::editor().close(&p);
        // Remove tasks from ring so they don't appear in project list
        if p.home_project.is_some() {
            projects.remove_from_ring(&p.name);
        }
    }
    projects.print();
}

pub fn pin_current() {
    let projects = projects::lock();
    if let Some(current) = projects.current() {
        let app = hammerspoon::current_application();
        drop(projects); // Release lock before modifying KV
        crate::kv::set_value_sync(&current.name, "land-in", app.as_str());
        hammerspoon::alert("📌");
        if debug() {
            crate::ps!("Pinned {}: land-in={}", current.name, app.as_str());
        }
    }
}

pub fn sprint() -> Response<Body> {
    let items = crate::status::get_sprint_status();
    let json = serde_json::to_string_pretty(&items).unwrap_or_default();
    Response::builder()
        .header("Content-Type", "application/json")
        .body(Body::from(json))
        .unwrap()
}

pub fn dashboard() -> Response<Body> {
    let items = crate::status::get_sprint_status();
    let jira_instance = std::env::var("JIRA_INSTANCE").ok();

    let cards_html: String = items
        .iter()
        .map(|item| render_card(item, jira_instance.as_deref()))
        .collect();

    let template = include_str!("dashboard.html");
    let html = template.replace("{{CARDS}}", &cards_html);

    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
}

fn render_card(item: &crate::status::SprintShowItem, jira_instance: Option<&str>) -> String {
    use crate::status::SprintShowItem;

    match item {
        SprintShowItem::Task(task) => {
            let jira_url = jira_instance
                .map(|i| format!("https://{}.atlassian.net/browse/{}", i, task.name))
                .unwrap_or_default();
            let key_html = if jira_url.is_empty() {
                format!(r#"<span class="card-key">{}</span>"#, task.name)
            } else {
                format!(
                    r#"<a class="card-key" href="{}" target="_blank">{}</a>"#,
                    jira_url, task.name
                )
            };

            let summary = task
                .jira
                .as_ref()
                .map(|j| html_escape(&j.summary))
                .unwrap_or_default();

            let status_html = task
                .jira
                .as_ref()
                .map(|j| {
                    format!(
                        r#"<span class="card-status">{} {}</span>"#,
                        j.status_emoji(),
                        html_escape(&j.status)
                    )
                })
                .unwrap_or_default();

            let pr_html = if let Some(ref pr) = task.pr {
                let comments = pr
                    .comments_display()
                    .map(|c| format!(" [{}]", html_escape(&c)))
                    .unwrap_or_default();
                format!(
                    r#"<span class="meta-item">PR: <a href="{}" target="_blank">{}</a>{}</span>"#,
                    pr.url,
                    html_escape(&pr.display()),
                    comments
                )
            } else {
                r#"<span class="meta-item">PR: <span class="cross">✗</span></span>"#.to_string()
            };

            let plan_html = if task.plan_exists {
                if let Some(ref url) = task.plan_url {
                    format!(
                        r#"<span class="meta-item">Plan: <a href="{}" target="_blank" class="check">✓</a></span>"#,
                        url
                    )
                } else {
                    r#"<span class="meta-item">Plan: <span class="check">✓</span></span>"#
                        .to_string()
                }
            } else {
                r#"<span class="meta-item">Plan: <span class="cross">✗</span></span>"#.to_string()
            };

            let iframe_html = match crate::serve_web::manager().get_or_start(&task.name, &task.path)
            {
                Ok(port) => {
                    let folder_encoded = url_encode(&task.path.to_string_lossy());
                    format!(
                        r#"<div class="card-actions"><button class="btn btn-terminal">Terminal</button><button class="btn btn-cursor">Cursor</button><button class="btn btn-vscode">VSCode</button><button class="btn btn-maximize">Maximize</button></div>
<div class="iframe-container"><iframe data-src="http://localhost:{}/?folder={}"></iframe></div>"#,
                        port, folder_encoded
                    )
                }
                Err(_) => String::new(),
            };

            let status_attr = task
                .jira
                .as_ref()
                .map(|j| status_data_attr(&j.status))
                .unwrap_or_default();

            format!(
                r#"<div class="card" data-task="{}"{}>
<div class="card-header">{}<span class="card-summary">{}</span>{}</div>
<div class="card-meta">{}{}</div>
{}
</div>"#,
                html_escape(&task.name),
                status_attr,
                key_html,
                summary,
                status_html,
                pr_html,
                plan_html,
                iframe_html
            )
        }
        SprintShowItem::Issue(issue) => {
            let jira_url = jira_instance
                .map(|i| format!("https://{}.atlassian.net/browse/{}", i, issue.key))
                .unwrap_or_default();
            let key_html = if jira_url.is_empty() {
                format!(r#"<span class="card-key">{}</span>"#, issue.key)
            } else {
                format!(
                    r#"<a class="card-key" href="{}" target="_blank">{}</a>"#,
                    jira_url, issue.key
                )
            };
            let status_html = format!(
                r#"<span class="card-status">{} {}</span>"#,
                issue.status_emoji(),
                html_escape(&issue.status)
            );

            let status_attr = status_data_attr(&issue.status);

            format!(
                r#"<div class="card"{}>
<div class="card-header">{}<span class="card-summary">{}</span>{}</div>
<div class="no-task">no wormhole task</div>
</div>"#,
                status_attr,
                key_html,
                html_escape(&issue.summary),
                status_html
            )
        }
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn status_data_attr(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "done" | "closed" | "resolved" => r#" data-status="done""#.to_string(),
        _ => String::new(),
    }
}

pub fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
