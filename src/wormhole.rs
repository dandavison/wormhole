use std::convert::Infallible;
use std::path::PathBuf;
use std::thread;

use crate::endpoints;
use crate::project::Project;
use crate::project_path::ProjectPath;
use crate::projects;
use crate::ps;
use hyper::{Body, Request, Response};
use url::form_urlencoded;

#[derive(Debug)]
pub enum Application {
    Editor,
    Terminal,
    Other,
}

#[derive(Debug)]
pub enum WindowAction {
    Focus,
    Raise,
}

#[derive(Debug)]
pub struct QueryParams {
    pub land_in: Option<Application>,
    pub line: Option<usize>,
    pub names: Vec<String>,
}

pub async fn service(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let uri = req.uri();
    let path = uri.path().to_string();
    if &path == "/favicon.ico" {
        return Ok(Response::new(Body::from("")));
    }
    let params = QueryParams::from_query(uri.query());
    ps!("\nRequest: {} {:?}", uri, params);
    if &path == "/list-projects/" {
        Ok(endpoints::list_projects())
    } else if let Some(path) = path.strip_prefix("/add-project/") {
        // An absolute path must have a double slash: /add-project//Users/me/file.rs
        Ok(endpoints::add_project(&path.trim(), params.names))
    } else if let Some(name) = path.strip_prefix("/remove-project/") {
        Ok(endpoints::remove_project(&name.trim()))
    } else {
        // wormhole uses the `hs` client to make a call to the hammerspoon
        // service. But one might also want to use hammerspoon to configure a
        // key binding to make a call to the wormhole service. In practice I
        // found that hammerspoon did not support this concurrency: it was
        // unable to handle the `hs` call from wormhole when it was still
        // waiting for its originating HTTP request to return. Instead the `hs`
        // call blocked until the HTTP request timed out. So, wormhole returns
        // immediately, performing its actions asynchronously.
        thread::spawn(move || switch_project(path, params.line, params.land_in));
        Ok(Response::new(Body::from("Sent into wormhole.")))
    }
}

fn switch_project(url_path: String, line: Option<usize>, mut land_in: Option<Application>) {
    let project_path = if url_path == "/previous-project/" {
        projects::previous().map(|p| p.as_project_path())
    } else if url_path == "/next-project/" {
        // TODO
        projects::previous().map(|p| p.as_project_path())
    } else if let Some(name) = url_path.strip_prefix("/project/") {
        Project::by_name(name).map(|p| p.as_project_path())
    } else if let Some(absolute_path) = url_path.strip_prefix("/file/") {
        ProjectPath::from_absolute_path(&PathBuf::from(absolute_path))
    } else if let Some(project_path) = ProjectPath::from_github_url(&url_path, line) {
        land_in = Some(Application::Editor);
        Some(project_path)
    } else {
        None
    };
    if let Some(project_path) = project_path {
        project_path.open(land_in)
    }
}

impl QueryParams {
    pub fn from_query(query: Option<&str>) -> Self {
        let mut params = QueryParams {
            land_in: None,
            line: None,
            names: vec![],
        };
        if let Some(query) = query {
            for (key, val) in
                form_urlencoded::parse(query.to_lowercase().as_bytes()).collect::<Vec<(_, _)>>()
            {
                if key == "land-in" {
                    if val == "terminal" {
                        params.land_in = Some(Application::Terminal);
                    } else if val == "editor" {
                        params.land_in = Some(Application::Editor);
                    }
                } else if key == "line" {
                    params.line = val.parse::<usize>().ok();
                } else if key == "name" {
                    params.names = val
                        .to_string()
                        .split(",")
                        .map(|s| s.trim().to_string())
                        .collect();
                }
            }
        }
        params
    }
}
