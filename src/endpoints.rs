use hyper::{Body, Response};
use itertools::Itertools;

use crate::{config, projects};

pub fn list_projects() -> Response<Body> {
    Response::new(Body::from(
        projects::lock()
            .names()
            .iter()
            .map(|s| s.as_str())
            .join("\n"),
    ))
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

pub fn close_project(name: &str) -> Response<Body> {
    // TODO: close editor workspace
    let projects = projects::lock();
    projects.by_name(name).map(|p| {
        config::TERMINAL.close(&p);
    });
    projects.print();
    Response::new(Body::from(format!("closed project: {}", name)))
}
