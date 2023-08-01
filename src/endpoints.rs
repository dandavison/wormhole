use hyper::{Body, Response};
use itertools::Itertools;

use crate::project;

pub fn list_projects() -> Response<Body> {
    Response::new(Body::from(
        project::list_project_names()
            .iter()
            .map(|s| s.as_str())
            .join("\n"),
    ))
}

pub fn add_project(path: &str) -> Response<Body> {
    let resp = if let Some(path) = path.strip_prefix("/add-project/") {
        project::add_project(path);
        format!("Added project: {}", path)
    } else {
        "Not an add-project URL".to_string()
    };
    Response::new(Body::from(resp))
}
