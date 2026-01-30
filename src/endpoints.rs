use hyper::{Body, Response, StatusCode};
use std::collections::HashSet;

use crate::project::ProjectKey;
use crate::{config, github, hammerspoon, jira, projects, util::debug};

/// Return JSON with current and available projects (including tasks)
/// If sprint_only is true, filter to tasks in the current sprint and include JIRA/PR info
pub fn list_projects(sprint_only: bool) -> Response<Body> {
    // Get sprint issue keys if filtering
    let sprint_keys: HashSet<String> = if sprint_only {
        jira::get_sprint_issues()
            .map(|issues| issues.into_iter().map(|i| i.key).collect())
            .unwrap_or_default()
    } else {
        HashSet::new()
    };

    // Get currently open projects
    let open_projects = projects::lock().open();

    let mut current: Vec<_> = open_projects
        .into_iter()
        .filter_map(|project| {
            let jira_key = project.kv.get("jira_key").cloned();

            // If sprint_only, filter to tasks with jira_key in sprint
            if sprint_only {
                let key = jira_key.as_ref()?;
                if !sprint_keys.contains(key) {
                    return None;
                }
            }

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

            // For sprint view, add JIRA status and PR info
            if sprint_only {
                if let Some(ref key) = jira_key {
                    if let Ok(Some(issue)) = jira::get_issue(key) {
                        obj["jira"] = serde_json::json!({
                            "key": issue.key,
                            "status": issue.status,
                            "summary": issue.summary,
                        });
                    }
                }
                let path = project
                    .worktree_path()
                    .unwrap_or_else(|| project.repo_path.clone());
                if let Some(pr) = github::get_pr_status(&path) {
                    obj["pr"] = serde_json::json!(pr);
                }
            }

            Some(obj)
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

    let mut json = serde_json::json!({ "current": current });

    if !sprint_only {
        let available = config::available_projects();
        let available: Vec<&str> = available.keys().map(|s| s.as_str()).collect();
        json["available"] = serde_json::json!(available);
    }

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
    use rayon::prelude::*;

    // Refresh tasks from filesystem
    projects::refresh_tasks();

    // Reload KV data from disk
    {
        let mut projects = projects::lock();
        crate::kv::load_kv_data(&mut projects);
    }

    // Refresh GitHub info for all projects concurrently
    let keys: Vec<_> = {
        let projects = projects::lock();
        projects.all().iter().map(|p| p.store_key()).collect()
    };

    keys.par_iter().for_each(|key| {
        let mut projects = projects::lock();
        if let Some(project) = projects.get_mut(key) {
            crate::github::refresh_github_info(project);
        }
    });

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
            // Primary identifier: repo branch
            let branch_html = task
                .branch
                .as_ref()
                .map(|b| format!(" {}", html_escape(b)))
                .unwrap_or_default();
            let repo_branch = format!(
                r#"<span class="card-key">{}{}</span>"#,
                html_escape(&task.name),
                branch_html
            );

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
                    r#"<span class="meta-item"><a href="{}" target="_blank">{}</a>{}</span>"#,
                    pr.url,
                    html_escape(&pr.display()),
                    comments
                )
            } else {
                String::new()
            };

            let jira_html = task
                .jira
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

            // Task identifier for switching: repo:branch
            let task_id = task
                .branch
                .as_ref()
                .map(|b| format!("{}:{}", task.name, b))
                .unwrap_or_else(|| task.name.clone());

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
