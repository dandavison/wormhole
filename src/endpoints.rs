use hyper::{Body, Response};

use crate::project::PROJECTS;

pub fn projects() -> Response<Body> {
    let mut project_names: Vec<_> = PROJECTS.get().unwrap().keys().cloned().collect();
    project_names.sort();
    Response::new(Body::from(project_names.join("\n")))
}
