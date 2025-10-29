use std::convert::Infallible;
use std::thread;

use crate::endpoints;
use crate::project_path::ProjectPath;
use crate::projects;
use crate::projects::Mutation;
use crate::ps;
use hyper::{header, Body, Request, Response, StatusCode};
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
    if &path != "/list-projects/" {
        ps!("\nRequest: {} {:?}", uri, params);
    }
    if &path == "/list-projects/" {
        Ok(endpoints::list_projects())
    } else if &path == "/debug-projects/" {
        Ok(endpoints::debug_projects())
    } else if let Some(path) = path.strip_prefix("/add-project/") {
        // An absolute path must have a double slash: /add-project//Users/me/file.rs
        Ok(endpoints::add_project(&path.trim(), params.names))
    } else if let Some(name) = path.strip_prefix("/remove-project/") {
        Ok(endpoints::remove_project(&name.trim()))
    } else if let Some(name) = path.strip_prefix("/open-project/") {
        Ok(endpoints::open_project(&name.trim()))
    } else if let Some(name) = path.strip_prefix("/close-project/") {
        Ok(endpoints::close_project(&name.trim()))
    } else {
        // wormhole uses the `hs` client to make a call to the hammerspoon
        // service. But one might also want to use hammerspoon to configure a
        // key binding to make a call to the wormhole service. In practice I
        // found that hammerspoon did not support this concurrency: it was
        // unable to handle the `hs` call from wormhole when it was still
        // waiting for its originating HTTP request to return. Instead the `hs`
        // call blocked until the HTTP request timed out. So, wormhole returns
        // immediately, performing its actions asynchronously.
        if let Some((Some(project_path), mutation, land_in)) =
            determine_requested_operation(&path, params.line, params.land_in)
        {
            if project_path.project.name != "dan" {
                thread::spawn(move || project_path.open(mutation, land_in));
                Ok(Response::new(Body::from("Sent into wormhole.")))
            } else {
                Ok(Response::new(Body::from("Error: dan is not allowed.")))
            }
        } else {
            let redirect_to = format!(
                "https://github.com{path}#L{}?wormhole=false",
                params.line.unwrap_or(1)
            );
            ps!("Redirecting to: {}", redirect_to);
            let response = Response::builder()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, redirect_to)
                .body(Body::empty())
                .unwrap();
            return Ok(response);
        }
    }
}

fn determine_requested_operation(
    url_path: &str,
    line: Option<usize>,
    land_in: Option<Application>,
) -> Option<(Option<ProjectPath>, Mutation, Option<Application>)> {
    let projects = projects::lock();
    if url_path == "/previous-project/" {
        let p = projects.previous().map(|p| p.as_project_path());
        Some((p, Mutation::RotateLeft, land_in))
    } else if url_path == "/next-project/" {
        let p = projects.next().map(|p| p.as_project_path());
        Some((p, Mutation::RotateRight, land_in))
    } else if let Some(name) = url_path.strip_prefix("/project/") {
        let p = projects.by_name(name).map(|p| p.as_project_path());
        Some((p, Mutation::Insert, land_in))
    } else if let Some(absolute_path) = url_path.strip_prefix("/file/") {
        let p = ProjectPath::from_absolute_path(absolute_path, &projects);
        Some((p, Mutation::Insert, land_in))
    } else if let Some(project_path) = ProjectPath::from_github_url(&url_path, line, &projects) {
        if url_path.ends_with(".md") {
            None
        } else {
            Some((
                Some(project_path),
                Mutation::Insert,
                Some(Application::Editor),
            ))
        }
    } else {
        None
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
