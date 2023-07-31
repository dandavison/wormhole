use hyper::{Body, Response};
use itertools::Itertools;

use crate::project::list_project_names;

pub fn projects() -> Response<Body> {
    Response::new(Body::from(
        list_project_names().iter().map(|s| s.as_str()).join("\n"),
    ))
}
