use std::collections::VecDeque;

use hyper::{Body, Response};

use crate::{config, hammerspoon, projects, task, util::debug};

/// Return JSON with current and available projects (including tasks)
pub fn list_projects() -> Response<Body> {
    let tasks = task::tasks();

    // Get currently open projects
    let mut current: VecDeque<_> = projects::lock()
        .open()
        .into_iter()
        .map(|p| {
            let mut obj = serde_json::json!({ "name": p.name });
            if let Some(home) = &p.home_project {
                obj["home_project"] = serde_json::json!(home);
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
        .body(Body::from(json.to_string()))
        .unwrap()
}

pub fn debug_projects() -> Response<Body> {
    let projects = projects::lock();
    let mut output = Vec::new();

    for (i, project) in projects.all().iter().enumerate() {
        let aliases = if project.aliases.is_empty() {
            "none".to_string()
        } else {
            project.aliases.join(", ")
        };
        output.push(format!(
            "[{}] name: {}, path: {}, aliases: [{}]",
            i,
            project.name,
            project.path.display(),
            aliases
        ));
    }

    Response::new(Body::from(output.join("\n")))
}

pub fn remove_project(name: &str) -> Response<Body> {
    let mut projects = projects::lock();
    projects.by_name(name).map(|p| {
        config::TERMINAL.close(&p);
    });
    projects.remove(name);

    projects.print();
    Response::new(Body::from(format!("removed project: {}", name)))
}

pub fn close_project(name: &str) {
    let projects = projects::lock();
    projects.by_name(name).map(|p| {
        config::TERMINAL.close(&p);
        config::EDITOR.close(&p);
    });
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
