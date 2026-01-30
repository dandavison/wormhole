use hyper::{Body, Response, StatusCode};

use crate::project::ProjectKey;
use crate::{config, hammerspoon, projects, util::debug};

/// Return JSON with current and available projects (including tasks)
/// Includes cached JIRA/PR status for tasks
pub fn list_projects() -> Response<Body> {
    let open_projects = projects::lock().open();

    let mut current: Vec<_> = open_projects
        .into_iter()
        .map(|project| {
            let mut obj = serde_json::json!({ "name": project.repo_name });
            if let Some(branch) = &project.branch {
                obj["branch"] = serde_json::json!(branch);
                if let Some(worktree_path) = project.worktree_path() {
                    obj["path"] = serde_json::json!(worktree_path);
                }
            } else {
                obj["path"] = serde_json::json!(project.repo_path);
            }
            if !project.kv.is_empty() {
                obj["kv"] = serde_json::json!(project.kv);
            }
            if let Some(ref jira) = project.cached_jira {
                obj["jira"] = serde_json::json!(jira);
            }
            if let Some(ref pr) = project.cached_pr {
                obj["pr"] = serde_json::json!(pr);
            }
            obj
        })
        .collect();

    // Sort: projects without branch first (alphabetically), then tasks (by name, branch)
    current.sort_by(|a, b| {
        let a_branch = a.get("branch").and_then(|h| h.as_str());
        let b_branch = b.get("branch").and_then(|h| h.as_str());
        let a_name = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let b_name = b.get("name").and_then(|n| n.as_str()).unwrap_or("");

        match (a_branch, b_branch) {
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, None) => a_name.cmp(b_name),
            (Some(ab), Some(bb)) => (a_name, ab).cmp(&(b_name, bb)),
        }
    });

    let available = config::available_projects();
    let available: Vec<&str> = available.keys().map(|s| s.as_str()).collect();

    let json = serde_json::json!({
        "current": current,
        "available": available,
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
                "name": project.repo_name,
                "path": project.repo_path.display().to_string(),
                "branch": project.branch,
            })
        })
        .collect();

    Response::builder()
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string_pretty(&output).unwrap()))
        .unwrap()
}

pub fn remove_project(name: &str) -> Response<Body> {
    let key = ProjectKey::parse(name);
    let mut projects = projects::lock();
    if let Some(p) = projects.by_key(&key) {
        config::TERMINAL.close(&p);
    }
    if projects.remove(&key) {
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
    let key = ProjectKey::parse(name);
    let mut projects = projects::lock();
    if let Some(p) = projects.by_key(&key) {
        config::TERMINAL.close(&p);
        config::editor().close(&p);
        // Remove tasks from ring so they don't appear in project list
        if p.is_task() {
            projects.remove_from_ring(&p.store_key());
        }
    }
    projects.print();
}

/// Refresh all in-memory data from external sources (fs, github)
pub fn refresh_all() {
    // Refresh tasks from filesystem
    projects::refresh_tasks();

    // Reload KV data from disk
    {
        let mut projects = projects::lock();
        crate::kv::load_kv_data(&mut projects);
    }

    // Refresh cached JIRA/PR status for all tasks (parallel via rayon)
    projects::refresh_cache();

    if debug() {
        let projects = projects::lock();
        projects.print();
    }
}

pub fn pin_current() {
    let projects = projects::lock();
    if let Some(current) = projects.current() {
        let app = hammerspoon::current_application();
        let key = current.store_key();
        drop(projects); // Release lock before modifying KV
        crate::kv::set_value_sync(&key, "land-in", app.as_str());
        hammerspoon::alert("📌");
        if debug() {
            crate::ps!("Pinned {}: land-in={}", key, app.as_str());
        }
    }
}

pub fn dashboard() -> Response<Body> {
    use crate::project::Project;

    let tasks: Vec<Project> = {
        let projects = projects::lock();
        projects.all().into_iter().filter(|p| p.is_task()).cloned().collect()
    };
    let jira_instance = std::env::var("JIRA_INSTANCE").ok();

    let cards_html: String = tasks
        .iter()
        .map(|task| render_task_card(task, jira_instance.as_deref()))
        .collect();

    let template = include_str!("dashboard.html");
    let html = template.replace("{{CARDS}}", &cards_html);

    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
}

fn render_task_card(task: &crate::project::Project, jira_instance: Option<&str>) -> String {
    let branch_html = task
        .branch
        .as_ref()
        .map(|b| format!(" {}", html_escape(b)))
        .unwrap_or_default();
    let repo_branch = format!(
        r#"<span class="card-key">{}{}</span>"#,
        html_escape(&task.repo_name),
        branch_html
    );

    let summary = task
        .cached_jira
        .as_ref()
        .map(|j| html_escape(&j.summary))
        .unwrap_or_default();

    let status_html = task
        .cached_jira
        .as_ref()
        .map(|j| {
            format!(
                r#"<span class="card-status">{} {}</span>"#,
                j.status_emoji(),
                html_escape(&j.status)
            )
        })
        .unwrap_or_default();

    let pr_html = if let Some(ref pr) = task.cached_pr {
        let comments = pr
            .comments_display()
            .map(|c| format!(" [{}]", html_escape(&c)))
            .unwrap_or_default();
        format!(
            r#"<span class="meta-item"><a href="{}" target="_blank">{}</a>{}</span>"#,
            pr.url,
            html_escape(&pr.display()),
            comments
        )
    } else {
        String::new()
    };

    let jira_html = task
        .cached_jira
        .as_ref()
        .and_then(|j| {
            jira_instance.map(|i| {
                format!(
                    r#"<span class="meta-item"><a href="https://{}.atlassian.net/browse/{}" target="_blank">{}</a></span>"#,
                    i,
                    html_escape(&j.key),
                    html_escape(&j.key)
                )
            })
        })
        .unwrap_or_default();

    let path = task.working_dir();
    let plan_exists = path.join(".task/plan.md").exists();
    let plan_url = if plan_exists {
        crate::git::github_file_url(&path, ".task/plan.md")
    } else {
        None
    };

    let plan_html = if plan_exists {
        if let Some(ref url) = plan_url {
            format!(
                r#"<span class="meta-item">Plan: <a href="{}" target="_blank" class="check">✓</a></span>"#,
                url
            )
        } else {
            r#"<span class="meta-item">Plan: <span class="check">✓</span></span>"#.to_string()
        }
    } else {
        r#"<span class="meta-item">Plan: <span class="cross">✗</span></span>"#.to_string()
    };

    let iframe_html = match crate::serve_web::manager().get_or_start(&task.repo_name, &path) {
        Ok(port) => {
            let folder_encoded = url_encode(&path.to_string_lossy());
            format!(
                r#"<div class="card-actions"><button class="btn btn-terminal">Terminal</button><button class="btn btn-cursor">Cursor</button><button class="btn btn-vscode">VSCode</button><button class="btn btn-maximize">Maximize</button></div>
<div class="iframe-container"><iframe data-src="http://localhost:{}/?folder={}"></iframe></div>"#,
                port, folder_encoded
            )
        }
        Err(_) => String::new(),
    };

    let status_attr = task
        .cached_jira
        .as_ref()
        .map(|j| status_data_attr(&j.status))
        .unwrap_or_default();

    let task_id = task.store_key().to_string();

    format!(
        r#"<div class="card" data-task="{}"{}>
<div class="card-header">{}<span class="card-summary">{}</span>{}</div>
<div class="card-meta">{}{}{}</div>
{}
</div>"#,
        html_escape(&task_id),
        status_attr,
        repo_branch,
        summary,
        status_html,
        jira_html,
        pr_html,
        plan_html,
        iframe_html
    )
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
