use std::collections::{HashSet, VecDeque};

use hyper::{Body, Response};

use crate::{config, hammerspoon, projects, util::debug};

/// Return JSON with current and available projects
pub fn list_projects() -> Response<Body> {
    // Get currently open projects
    let mut current: VecDeque<_> = projects::lock()
        .open()
        .into_iter()
        .map(|p| p.name)
        .collect();
    if !current.is_empty() {
        // Rotate so current project is last (for selector UIs)
        current.rotate_left(1);
    }

    // Get available projects
    let mut available: Vec<String> = Vec::new();
    for search_dir in config::search_paths() {
        if let Ok(entries) = std::fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        if !name.starts_with('.')
                            && !available.contains(&name.to_string())
                            && !config::is_excluded(name)
                        {
                            available.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    available.sort();

    let current: Vec<&str> = current.iter().map(|s| s.as_str()).collect();
    let available: Vec<&str> = available.iter().map(|s| s.as_str()).collect();

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
        if debug() {
            crate::ps!("Pinned {}: land-in={}", current.name, app.as_str());
        }
    }
}
