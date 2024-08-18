use hyper::{Body, Response};
use itertools::Itertools;

use crate::projects;

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
    projects.remove(name);
    projects.print();
    Response::new(Body::from(format!("removed project: {}", name)))
}
