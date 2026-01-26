use std::collections::VecDeque;

use hyper::{Body, Response};

use crate::{config, git, hammerspoon, projects, task, util::debug};

/// Return JSON with current and available projects (including tasks)
pub fn list_projects() -> Response<Body> {
    let tasks = task::tasks();

    // Get currently open projects, using task info where available
    let mut current: VecDeque<_> = projects::lock()
        .open()
        .into_iter()
        .map(|p| {
            let project = tasks.get(&p.name).unwrap_or(&p);
            let mut obj = serde_json::json!({ "name": project.name });
            if let Some(home) = &project.home_project {
                obj["home_project"] = serde_json::json!(home);
                if let Some(branch) = git::current_branch(&project.path) {
                    obj["branch"] = serde_json::json!(branch);
                }
            }
            obj
        })
        .collect();
    if !current.is_empty() {
        current.rotate_left(1);
    }

    // Add discovered tasks that aren't already open
    let open_names: std::collections::HashSet<_> = current
        .iter()
        .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
        .map(|s| s.to_string())
        .collect();

    for (task_name, task_project) in &tasks {
        if !open_names.contains(task_name) {
            let mut obj = serde_json::json!({ "name": task_name });
            if let Some(home) = &task_project.home_project {
                obj["home_project"] = serde_json::json!(home);
                if let Some(branch) = git::current_branch(&task_project.path) {
                    obj["branch"] = serde_json::json!(branch);
                }
            }
            current.push_back(obj);
        }
    }

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
    if let Some(p) = projects.resolve(name) {
        config::TERMINAL.close(&p);
    }
    projects.remove(name);
    projects.print();
    Response::new(Body::from(format!("removed project: {}", name)))
}

pub fn close_project(name: &str) {
    let projects = projects::lock();
    if let Some(p) = projects.resolve(name) {
        config::TERMINAL.close(&p);
        config::EDITOR.close(&p);
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

    let html = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Wormhole Sprint</title>
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{
    font-family: "SF Mono", "Menlo", "Monaco", monospace;
    background-color: #fdfeff;
    background-image:
        linear-gradient(#eef1f4 1px, transparent 1px),
        linear-gradient(90deg, #eef1f4 1px, transparent 1px);
    background-size: 20px 20px;
    min-height: 100vh;
    padding: 3rem 2rem 2rem 3rem;
}}
header {{
    margin-bottom: 2rem;
    padding-bottom: 1rem;
    border-bottom: 2px solid #333;
}}
h1 {{
    font-size: 1.25rem;
    font-weight: 600;
    letter-spacing: 0.1em;
    color: #333;
}}
.grid {{
    display: flex;
    flex-direction: column;
    gap: 1rem;
}}
.card {{
    background: #fff;
    border: 1px solid #ccc;
    padding: 1rem;
    box-shadow: 2px 2px 0 #ddd;
}}
.card[data-task] {{
    cursor: pointer;
    transition: border-color 0.1s, box-shadow 0.1s;
}}
.card[data-task]:hover {{
    border-color: #0066cc;
    box-shadow: 2px 2px 0 #0066cc;
}}
.card[data-task].switching {{
    opacity: 0.6;
}}
.card-header {{
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 0.5rem;
}}
.card-key {{
    font-weight: normal;
    font-size: 0.9rem;
    color: #1a1a1a;
    text-decoration: none;
}}
.card-key:hover {{ text-decoration: underline; }}
.card-status {{
    font-size: 0.75rem;
    padding: 0.125rem 0.5rem;
    border-radius: 2px;
    background: #eee;
    color: #555;
}}
.card-summary {{
    font-weight: 600;
    font-size: 0.85rem;
    color: #444;
    margin-bottom: 0.75rem;
    line-height: 1.4;
}}
.card-meta {{
    font-size: 0.75rem;
    color: #666;
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
}}
.meta-item {{ display: flex; align-items: center; gap: 0.25rem; }}
.meta-item a {{ color: #0066cc; text-decoration: none; }}
.meta-item a:hover {{ text-decoration: underline; }}
.check {{ color: #22863a; }}
.cross {{ color: #999; }}
.no-task {{
    font-size: 0.7rem;
    color: #888;
    font-style: italic;
    margin-top: 0.5rem;
}}
.card-actions {{
    margin-top: 0.75rem;
    display: flex;
    gap: 0.5rem;
}}
.btn-open {{
    font-family: inherit;
    font-size: 0.75rem;
    padding: 0.25rem 0.75rem;
    border: 1px solid #0066cc;
    background: #fff;
    color: #0066cc;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
}}
.btn-open:hover {{
    background: #0066cc;
    color: #fff;
}}
.btn-open.active {{
    background: #0066cc;
    color: #fff;
}}
.btn-maximize {{
    font-family: inherit;
    font-size: 0.75rem;
    padding: 0.25rem 0.75rem;
    border: 1px solid #666;
    background: #fff;
    color: #666;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    display: none;
}}
.btn-maximize:hover {{
    background: #666;
    color: #fff;
}}
.iframe-container.expanded ~ .card-actions .btn-maximize,
.card.expanded .btn-maximize {{
    display: inline-block;
}}
.iframe-container {{
    display: none;
    margin-top: 0.75rem;
    border: 1px solid #ccc;
}}
.iframe-container.expanded {{
    display: block;
}}
.iframe-container iframe {{
    width: 100%;
    height: 600px;
    border: none;
}}
.card.expanded {{
    max-width: none;
}}
.card.maximized {{
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 1000;
    max-width: none;
    margin: 0;
    border-radius: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}}
.card.maximized .iframe-container {{
    flex: 1;
    margin-top: 0;
    border: none;
}}
.card.maximized .iframe-container iframe {{
    height: 100%;
}}
.card.maximized .card-actions {{
    position: absolute;
    top: 0.5rem;
    right: 0.5rem;
    margin: 0;
    z-index: 1001;
}}
.card.maximized > *:not(.card-actions):not(.iframe-container) {{
    display: none;
}}
</style>
</head>
<body>
<div class="grid">{}</div>
<script>
document.querySelectorAll('.btn-open').forEach(btn => {{
    btn.addEventListener('click', e => {{
        e.stopPropagation();
        const card = btn.closest('.card');
        const container = card.querySelector('.iframe-container');
        const iframe = container.querySelector('iframe');
        const isExpanding = !container.classList.contains('expanded');

        container.classList.toggle('expanded');
        card.classList.toggle('expanded');
        btn.classList.toggle('active');

        if (isExpanding) {{
            if (!iframe.src) {{
                iframe.src = iframe.dataset.src;
            }}
            fetch('/project/switch/' + card.dataset.task + '?skip-editor=true');
        }}
    }});
}});

document.querySelectorAll('.btn-maximize').forEach(btn => {{
    btn.addEventListener('click', e => {{
        e.stopPropagation();
        const card = btn.closest('.card');
        const isMaximized = card.classList.toggle('maximized');
        btn.textContent = isMaximized ? 'Restore' : 'Maximize';
        document.body.style.overflow = isMaximized ? 'hidden' : '';
    }});
}});

document.addEventListener('keydown', e => {{
    if (e.key === 'Escape') {{
        const maximized = document.querySelector('.card.maximized');
        if (maximized) {{
            maximized.classList.remove('maximized');
            maximized.querySelector('.btn-maximize').textContent = 'Maximize';
            document.body.style.overflow = '';
        }}
    }}
}});
</script>
</body>
</html>"##,
        cards_html
    );

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
                format!(
                    r#"<span class="meta-item">PR: <a href="{}" target="_blank">{}</a></span>"#,
                    pr.url,
                    html_escape(&pr.display())
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
                        r#"<div class="card-actions"><button class="btn-open">Open</button><button class="btn-maximize">Maximize</button></div>
<div class="iframe-container"><iframe data-src="http://localhost:{}/?folder={}"></iframe></div>"#,
                        port, folder_encoded
                    )
                }
                Err(_) => String::new(),
            };

            format!(
                r#"<div class="card" data-task="{}">
<div class="card-header">{}{}</div>
<div class="card-summary">{}</div>
<div class="card-meta">{}{}</div>
{}
</div>"#,
                html_escape(&task.name),
                key_html,
                status_html,
                summary,
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

            format!(
                r#"<div class="card">
<div class="card-header">{}{}</div>
<div class="card-summary">{}</div>
<div class="no-task">no wormhole task</div>
</div>"#,
                key_html,
                status_html,
                html_escape(&issue.summary)
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

fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
