use std::{collections::VecDeque, path::PathBuf};

use hyper::{Body, Response};
use itertools::Itertools;

use crate::{config, projects};

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

pub fn add_project(path: &str, names: Vec<String>) -> Response<Body> {
    let mut resp = path.to_string();
    if !names.is_empty() {
        resp = format!("{} -> {}", resp, names.join(", "));
    }
    let mut projects = projects::lock();
    projects.add(path, names);
    projects.print();
    Response::new(Body::from(resp))
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

pub fn open_project(name_or_path: &str) -> Response<Body> {
    // Look up project by name without continuing to hold lock.
    let project = {
        let projects = projects::lock();
        projects
            .by_name(name_or_path)
            .or_else(|| projects.by_path(&PathBuf::from(name_or_path)))
    };

    if let Some(p) = project {
        p.root().open(
            projects::Mutation::Insert,
            Some(crate::wormhole::Application::Terminal),
        );
        Response::new(Body::from(format!("opened project: {}", name_or_path)))
    } else {
        Response::builder()
            .status(hyper::StatusCode::NOT_FOUND)
            .body(Body::from(format!("project not found: {}", name_or_path)))
            .unwrap()
    }
}

pub fn close_project(name: &str) {
    let projects = projects::lock();
    projects.by_name(name).map(|p| {
        config::TERMINAL.close(&p);
        config::EDITOR.close(&p);
    });
    projects.print();
}
