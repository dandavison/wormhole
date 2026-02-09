// /project/* HTTP handlers.
// Handlers must do no I/O (no subprocess calls, no filesystem access).
// All data should come from in-memory caches populated by refresh_* functions.

use hyper::{Body, Response, StatusCode};
use std::thread;
use std::time::{Duration, Instant};

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
            .filter(|p| p.is_active(&window_names))
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

fn remove_project(name: &str) -> Response<Body> {
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

fn close_project(name: &str) {
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

pub fn neighbors(active: bool) -> Response<Body> {
    let projects = projects::lock();
    let ring: Vec<serde_json::Value> = if active {
        let window_names = crate::tmux::window_names();
        projects
            .open()
            .into_iter()
            .filter(|p| p.is_active(&window_names))
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
    let active_keys: Option<std::collections::HashSet<String>> = if params.active {
        Some(crate::tmux::window_names().into_iter().collect())
    } else {
        None
    };

    let p = {
        let mut projects = projects::lock();
        let ring_len = projects.keys().len();
        let mutation = match direction {
            Direction::Previous => Mutation::RotateLeft,
            Direction::Next => Mutation::RotateRight,
        };

        let mut result = None;
        for _ in 0..ring_len {
            let candidate = match direction {
                Direction::Previous => projects.previous(),
                Direction::Next => projects.next(),
            };
            if let Some(ref p) = candidate {
                projects.apply(mutation.clone(), &p.store_key());
                let excluded = active_keys
                    .as_ref()
                    .is_some_and(|active| !active.contains(&p.store_key().to_string()));
                if !excluded {
                    result = Some(p.as_project_path());
                    break;
                }
            } else {
                break;
            }
        }
        result
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
            .body(Body::from(super::WORMHOLE_RESPONSE_HTML))
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
                    let folder_encoded = super::url_encode(&project_path.to_string_lossy());
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

/// Generic long-poll helper: waits until predicate returns true or timeout.
/// Returns true if predicate became true, false if timeout.
pub async fn poll_until<F>(mut predicate: F, timeout: Duration) -> bool
where
    F: FnMut() -> bool,
{
    // Check immediately
    if predicate() {
        return true;
    }

    let deadline = Instant::now() + timeout;
    let mut rx = projects::subscribe_to_changes();

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }

        // Wait for state change or timeout
        match tokio::time::timeout(remaining, rx.changed()).await {
            Ok(Ok(())) => {
                if predicate() {
                    return true;
                }
            }
            _ => return false, // timeout or channel closed
        }
    }
}

/// Long-poll endpoint for current project changes.
pub async fn poll_current(client_current: Option<&str>, timeout_secs: u64) -> Response<Body> {
    let client_current = client_current.map(|s| s.to_string());
    let changed = poll_until(
        || {
            let server_current = projects::lock()
                .current()
                .map(|p| p.store_key().to_string());
            match (&client_current, &server_current) {
                (None, None) => false,
                (Some(c), Some(s)) => c != s,
                _ => true,
            }
        },
        Duration::from_secs(timeout_secs),
    )
    .await;

    let server_current = projects::lock()
        .current()
        .map(|p| p.store_key().to_string());
    poll_current_response(server_current, changed, timeout_secs)
}

fn poll_current_response(current: Option<String>, changed: bool, wait: u64) -> Response<Body> {
    let json = serde_json::json!({
        "current": current,
        "changed": changed
    });
    Response::builder()
        .header("Content-Type", "application/json")
        .header("Preference-Applied", format!("wait={}", wait))
        .body(Body::from(json.to_string()))
        .unwrap()
}
