// Handlers in this module must do no I/O (no subprocess calls, no filesystem access).
// All data should come from in-memory caches populated by refresh_* functions.
// This ensures fast response times for the HTTP API.

use hyper::{Body, Request, Response, StatusCode};
use std::thread;

use crate::project::ProjectKey;
use crate::project_path::ProjectPath;
use crate::projects::Mutation;
use crate::wormhole::QueryParams;
use crate::{config, hammerspoon, projects, util::debug};

/// Return JSON with current and available projects (including tasks)
/// Includes cached JIRA/PR status for tasks
/// If active=true, only returns projects with tmux windows
pub fn list_projects(active_only: bool) -> Response<Body> {
    let open_projects = if active_only {
        let window_names = crate::tmux::window_names();
        projects::lock()
            .open()
            .into_iter()
            .filter(|p| window_names.contains(&p.store_key().to_string()))
            .collect()
    } else {
        projects::lock().open()
    };

    let mut current: Vec<_> = open_projects
        .into_iter()
        .map(|project| {
            let mut obj = serde_json::json!({
                "project_key": project.store_key().to_string()
            });
            let path = project
                .worktree_path()
                .unwrap_or_else(|| project.repo_path.clone());
            obj["path"] = serde_json::json!(path);
            if !project.kv.is_empty() {
                obj["kv"] = serde_json::json!(project.kv);
            }
            if let Some(ref jira) = project.cached.jira {
                obj["jira"] = serde_json::json!(jira);
            }
            if let Some(ref pr) = project.cached.pr {
                obj["pr"] = serde_json::json!(pr);
            }
            obj
        })
        .collect();

    // Sort: projects (no colon) first alphabetically, then tasks (with colon) by key
    current.sort_by(|a, b| {
        let a_key = a.get("project_key").and_then(|k| k.as_str()).unwrap_or("");
        let b_key = b.get("project_key").and_then(|k| k.as_str()).unwrap_or("");
        let a_is_task = a_key.contains(':');
        let b_is_task = b_key.contains(':');

        match (a_is_task, b_is_task) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            _ => a_key.cmp(b_key),
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
                "project_key": project.store_key().to_string(),
                "path": project.repo_path.display().to_string(),
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
        projects
            .all()
            .into_iter()
            .filter(|p| p.is_task())
            .cloned()
            .collect()
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
        .map(|b| format!(" {}", html_escape(b.as_str())))
        .unwrap_or_default();
    let repo_branch = format!(
        r#"<span class="card-key">{}{}</span>"#,
        html_escape(task.repo_name.as_str()),
        branch_html
    );

    let summary = task
        .cached
        .jira
        .as_ref()
        .map(|j| html_escape(&j.summary))
        .unwrap_or_default();

    let status_html = task
        .cached
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

    let pr_html = if let Some(ref pr) = task.cached.pr {
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
        .cached
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

    let path = task.working_tree();
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

    let iframe_html = match crate::serve_web::manager().get_or_start(task.repo_name.as_str(), &path)
    {
        Ok(port) => {
            let folder_encoded = url_encode(&path.to_string_lossy());
            format!(
                r#"<div class="card-actions"><button class="btn btn-icon btn-terminal" title="Terminal"><img src="data:image/png;base64,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" alt="Terminal"></button><button class="btn btn-icon btn-cursor" title="Cursor"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAMAAACdt4HsAAAAIGNIUk0AAHomAACAhAAA+gAAAIDoAAB1MAAA6mAAADqYAAAXcJy6UTwAAAGhUExURff39Pn59vj49dPTz42Mh/r69+Pj4JOTjj8+OCMiG+/v662tqFFQSiYlHiQjHPX18sTEwGdmYCsqJGdmYcXEwPb28tjY1IB/ejU0LSUkHYCAeufn45qalUNCPENDPPHx7rSzr1ZWUCcmH8rKxm1tZy4tJm5tZ9zc2YeHgTg3MYeHgurq56GhnEhHQfPz8Lq6tlxcVignIN3d2XV1by8uJyEgGICAeyEgGT08NVJRS09OSDs6M/X18WloY1ZVT76+uurq5uzs6O3t6qSjn2ppYyIhGjo5MomIg97e2vPy73JxbCAfGElIQaKinba2sikoIV1cVru6tujo5VlYUjAvKXV0b9DQzJ2dmI+OieHh3dra1kVEPk5NR6mppPb284OCfSopImNiXcHBvcfHwzY1LzMyK3x7dtXV0fDw7EFAOZaWkeXl4fn597CvqywrJFNSTLe2skdHQD8+N97d2mNjXeHh3o+Piru7tzAvKElIQoiIg8vLxzk4MW5uaPz8+bW0sN/f2+np5ldWUODg3EFAOsC/u0ZFPvb18lpZU42Nh/////7Hc1oAAAABYktHRIqFaHd2AAAACXBIWXMAAA8uAAAPLgEh0EwaAAAAB3RJTUUH6gEeEiU4Kf6EPwAAACV0RVh0ZGF0ZTpjcmVhdGUAMjAyNi0wMS0zMFQxODozNDo1MCswMDowMOXwHaoAAAAldEVYdGRhdGU6bW9kaWZ5ADIwMjUtMDktMTBUMDk6MTk6MTArMDA6MDAZCYFsAAAAKHRFWHRkYXRlOnRpbWVzdGFtcAAyMDI2LTAxLTMwVDE4OjM3OjU2KzAwOjAwS18K8AAAApNJREFUWMPtl+lXEzEUxSeBlAZIBkFrw2KhWAoWCl2gVVuX2iouCCqCCyCigICKu7jirvzXZlK6AJNZP+nhnn5pTvM7efck790qyr7+fwEuF9shrKmtrYHQ6XbkqfNi7K3zIEcIVN/QSCjhn8aGemS/eLXpQDMmQri55eAhe1ZA4Dvsx4xsi2F/qw9YrwOitvYOWt4uEPRIe5tVK1CgsytIyS7RYFdnwIoVQOk+GsJERzjU062YWQFBuLcPM6IrhvuOhY2tQJH+ASrZXrRioD8irwNEB4dilBiKxoYGo5I6YDyRHMamGk4m4pIyRlLp4yfMdTI1or8/k02dOn3mbMREuXM0m9EFqPkCO39hFAFoqIuXLudVGYAwdmUsYnhd0NVkwQDAbR6fuBaVXxd4/QY2BvAbN3lzCsoQYDpGzAD8xqVv3dZ/OeDOXWoO4HWwmdmcnhVz9/hFswDgiPn7C+qeOtCDh8wigFuxuLS86+XAR3ntnVoE8Jezsrr2eMcBnjxlNgBaP11/Nld1gLV10SisA/hvn3sqVYDV7UUbAPriZdlJsLxC7QO8o2WAukQdnKACQAuLzA0A5l6VWq0zAJqdJ24A4PUMdQWAbyrN2gkATKWpK0A0UTWsZIBMVgpAbyerxk1B0lR5W2f6ABiZqDoAk7V1Plg2sC4AjI1XlvCGdLDojDYBAPF35VWj0aboDNfiCd6XlkyGq7J3vGsA9OFjcYGP996wadLhAaOnEjA0QOBT8aulgCHqqIo4HPB5M6QdwHLEUXaELOr98vUb1orvsB6yhBW+VhHzqPf7dNB2zBNWqE0tPGjSHz9/US1o/rafuUXUxf4/zFnUVUphu+A4bCsi7m9tOY/7wgp3fzj29a/oL09sk0pvLkBgAAAAAElFTkSuQmCC" alt="Cursor"></button><button class="btn btn-icon btn-vscode" title="VSCode"><img src="data:image/png;base64,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" alt="VSCode"></button><button class="btn btn-maximize">Maximize</button></div>
<div class="iframe-container"><iframe data-src="http://localhost:{}/?folder={}"></iframe></div>"#,
                port, folder_encoded
            )
        }
        Err(_) => String::new(),
    };

    let status_attr = task
        .cached
        .jira
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

pub fn neighbors(active: bool) -> Response<Body> {
    let projects = projects::lock();
    let ring: Vec<serde_json::Value> = if active {
        let window_names = crate::tmux::window_names();
        projects
            .open()
            .into_iter()
            .filter(|p| window_names.contains(&p.store_key().to_string()))
            .map(|p| serde_json::json!({ "project_key": p.store_key().to_string() }))
            .collect()
    } else {
        projects
            .all()
            .iter()
            .map(|p| serde_json::json!({ "project_key": p.store_key().to_string() }))
            .collect()
    };
    let json = serde_json::json!({ "ring": ring });
    Response::new(Body::from(json.to_string()))
}

pub fn shell_env(pwd: Option<&str>) -> Response<Body> {
    let shell_code = pwd
        .map(|pwd| {
            let path = std::path::Path::new(pwd);
            let projects = projects::lock();
            projects
                .by_path(path)
                .map(|p| crate::terminal::shell_env_code(&p))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    Response::new(Body::from(shell_code))
}

pub fn navigate(direction: Direction, params: &QueryParams) {
    let p = {
        let mut projects = projects::lock();
        let (pp, mutation) = match direction {
            Direction::Previous => (projects.previous(), Mutation::RotateLeft),
            Direction::Next => (projects.next(), Mutation::RotateRight),
        };
        let pp = pp.map(|p| p.as_project_path());
        if let Some(ref pp) = pp {
            projects.apply(mutation, &pp.project.store_key());
        }
        pp
    };
    if let Some(project_path) = p {
        let land_in = params.land_in.clone();
        let skip_editor = params.skip_editor;
        thread::spawn(move || project_path.open_with_options(Mutation::None, land_in, skip_editor));
    }
}

pub enum Direction {
    Previous,
    Next,
}

pub fn remove(name: &str) -> Response<Body> {
    let name = name.trim();
    if let Some((repo, branch)) = name.split_once(':') {
        if let Some(task) = crate::task::get_task_by_branch(repo, branch) {
            if task.is_task() {
                return match crate::task::remove_task(repo, branch) {
                    Ok(()) => Response::new(Body::from(format!("Removed task: {}", name))),
                    Err(e) => Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from(e))
                        .unwrap(),
                };
            }
        }
    }
    remove_project(name)
}

pub fn close(name: &str) {
    let name = name.trim().to_string();
    thread::spawn(move || close_project(&name));
}

pub fn show(name: Option<&str>) -> Response<Body> {
    let status = match name.filter(|s| !s.is_empty()) {
        Some(n) => crate::status::get_status_by_name(n),
        None => crate::status::get_current_status(),
    };
    match status {
        Some(s) => {
            let json = serde_json::to_string_pretty(&s).unwrap_or_default();
            Response::builder()
                .header("Content-Type", "application/json")
                .body(Body::from(json))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Project not found"))
            .unwrap(),
    }
}

pub async fn describe(req: Request<Body>) -> Response<Body> {
    let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();
    let request: Result<crate::describe::DescribeRequest, _> = serde_json::from_slice(&body_bytes);
    match request {
        Ok(req) => {
            let response = crate::describe::describe(&req);
            let json = serde_json::to_string_pretty(&response).unwrap_or_default();
            Response::builder()
                .header("Content-Type", "application/json")
                .body(Body::from(json))
                .unwrap()
        }
        Err(e) => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(format!("Invalid JSON: {}", e)))
            .unwrap(),
    }
}

pub fn refresh_project(name: &str) -> Response<Body> {
    let key = ProjectKey::parse(name.trim());
    let mut projects = projects::lock();
    if let Some(project) = projects.get_mut(&key) {
        crate::github::refresh_github_info(project);
        let json = serde_json::json!({
            "project_key": project.store_key().to_string(),
            "github_pr": project.cached.github_pr,
            "github_repo": project.cached.github_repo,
        });
        Response::builder()
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string_pretty(&json).unwrap()))
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", key)))
            .unwrap()
    }
}

pub fn create_task(branch: &str, home_project: Option<&str>) -> Response<Body> {
    let branch = branch.trim();
    let repo = match home_project {
        Some(r) => r,
        None => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("home-project query param required"))
                .unwrap()
        }
    };
    match crate::task::create_task(repo, branch) {
        Ok(task) => Response::new(Body::from(format!("Created task: {}", task.store_key()))),
        Err(e) => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from(e))
            .unwrap(),
    }
}

pub fn switch(name_or_path: &str, params: &QueryParams, sync: bool) -> Response<Body> {
    let name_or_path = name_or_path.trim().to_string();
    let repo = params.home_project.clone();
    let branch = params.branch.clone();
    let land_in = params.land_in.clone();
    let skip_editor = params.skip_editor;
    let focus_terminal = params.focus_terminal;

    let do_switch = move || -> Result<(), String> {
        if let (Some(repo), Some(branch)) = (repo.as_ref(), branch.as_ref()) {
            return crate::task::open_task(repo, branch, land_in, skip_editor, focus_terminal);
        }
        if let Some((repo, branch)) = name_or_path.split_once(':') {
            return crate::task::open_task(repo, branch, land_in, skip_editor, focus_terminal);
        }
        let project_path = {
            let mut projects = projects::lock();
            resolve_project(&mut projects, &name_or_path)?
        };
        match project_path {
            Some(pp) => {
                pp.open(Mutation::Insert, land_in);
                Ok(())
            }
            None => Err(format!("Project '{}' not found", name_or_path)),
        }
    };

    if sync {
        match do_switch() {
            Ok(()) => Response::new(Body::from("ok")),
            Err(e) => Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(e))
                .unwrap(),
        }
    } else {
        thread::spawn(move || {
            if let Err(e) = do_switch() {
                crate::util::error(&e);
            }
        });
        Response::builder()
            .header("Content-Type", "text/html")
            .body(Body::from(WORMHOLE_RESPONSE_HTML))
            .unwrap()
    }
}

pub fn vscode_url(name: &str) -> Response<Body> {
    let key = ProjectKey::parse(name.trim());
    let result = {
        let projects = projects::lock();
        projects
            .by_key(&key)
            .map(|p| (p.repo_name.to_string(), p.repo_path.clone()))
    };

    match result {
        Some((project_name, project_path)) => {
            match crate::serve_web::manager().get_or_start(&project_name, &project_path) {
                Ok(port) => {
                    let folder_encoded = url_encode(&project_path.to_string_lossy());
                    let url = format!("http://localhost:{}/?folder={}", port, folder_encoded);
                    Response::builder()
                        .header("Content-Type", "application/json")
                        .body(Body::from(serde_json::json!({ "url": url }).to_string()))
                        .unwrap()
                }
                Err(e) => Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from(format!("Failed to start VSCode server: {}", e)))
                    .unwrap(),
            }
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", key)))
            .unwrap(),
    }
}

fn resolve_project(
    projects: &mut projects::Projects,
    name_or_path: &str,
) -> Result<Option<ProjectPath>, String> {
    let key = ProjectKey::parse(name_or_path);
    if let Some(project) = projects.by_key(&key) {
        Ok(Some(project.as_project_path()))
    } else if name_or_path.starts_with('/') {
        let path = std::path::PathBuf::from(name_or_path);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(name_or_path);
        let key = ProjectKey::project(name);
        if projects.by_key(&key).is_none() {
            projects.add(name_or_path, None)?;
        }
        Ok(projects.by_key(&key).map(|p| p.as_project_path()))
    } else if let Some(path) = config::resolve_project_name(name_or_path) {
        let path_str = path.to_string_lossy().to_string();
        projects.add(&path_str, Some(name_or_path))?;
        Ok(projects.by_key(&key).map(|p| p.as_project_path()))
    } else {
        Ok(None)
    }
}

pub const WORMHOLE_RESPONSE_HTML: &str =
    "<html><body><script>window.close()</script>Sent into wormhole.</body></html>";
