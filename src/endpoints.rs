use std::collections::VecDeque;

use hyper::{Body, Response};
use itertools::Itertools;

use crate::{config, hammerspoon, projects, util::debug};

pub fn list_projects() -> Response<Body> {
    let mut names: VecDeque<_> = projects::lock()
        .open()
        .into_iter()
        .map(|p| p.name)
        .collect();
    if !names.is_empty() {
        // These names will be used by selector UIs; rotate so that current
        // project is last.
        names.rotate_left(1);
    }
    Response::new(Body::from(names.iter().map(|s| s.as_str()).join("\n")))
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
