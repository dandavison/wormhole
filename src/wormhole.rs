use std::convert::Infallible;
use std::thread;

use crate::config;
use crate::endpoints;
use crate::project_path::ProjectPath;
use crate::projects;
use crate::projects::Mutation;
use crate::ps;
use hyper::{header, Body, Method, Request, Response, StatusCode};
use url::form_urlencoded;

#[derive(Clone, Debug)]
pub enum Application {
    Editor,
    Terminal,
}

impl Application {
    pub fn as_str(&self) -> &'static str {
        match self {
            Application::Editor => "editor",
            Application::Terminal => "terminal",
        }
    }
}

#[derive(Debug)]
pub struct QueryParams {
    pub land_in: Option<Application>,
    pub line: Option<usize>,
    pub names: Vec<String>,
    pub home: Option<String>,
}

pub async fn service(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let method = req.method().clone();
    let uri = req.uri();
    let path = uri.path().to_string();
    if &path == "/favicon.ico" {
        return Ok(Response::new(Body::from("")));
    }
    let params = QueryParams::from_query(uri.query());
    if &path != "/projects" {
        ps!("{} {} {:?}", method, uri, params);
    }

    // Collection endpoints
    if path == "/projects" {
        Ok(endpoints::list_projects())
    } else if path == "/tasks" {
        Ok(endpoints::list_tasks())
    } else if path == "/debug" {
        Ok(endpoints::debug_projects())
    } else if path == "/pin" {
        if method != Method::POST {
            return Ok(method_not_allowed("POST", "/pin"));
        }
        thread::spawn(move || endpoints::pin_current());
        Ok(Response::new(Body::from("Pinning current state...")))
    } else if let Some(rest) = path.strip_prefix("/project/") {
        handle_project_request(&method, rest, &params).await
    } else if let Some(rest) = path.strip_prefix("/task/") {
        handle_task_request(&method, rest, &params)
    } else if path == "/kv" {
        Ok(crate::kv::get_all_kv())
    } else if let Some(kv_path) = path.strip_prefix("/kv/") {
        handle_kv_request(&method, kv_path, req).await
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
            thread::spawn(move || project_path.open(mutation, land_in));
            Ok(Response::builder()
                .header("Content-Type", "text/html")
                .body(Body::from(
                    "<html><body><script>window.close()</script>Sent into wormhole.</body></html>",
                ))
                .unwrap())
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

fn method_not_allowed(expected: &str, path: &str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .body(Body::from(format!(
            "Method not allowed. Use {} for {}",
            expected, path
        )))
        .unwrap()
}

async fn handle_project_request(
    method: &Method,
    rest: &str,
    params: &QueryParams,
) -> Result<Response<Body>, Infallible> {
    // /project/previous - navigate to previous project
    if rest == "previous" {
        let projects = projects::lock();
        if let Some(project) = projects.previous() {
            let project_path = project.as_project_path();
            let land_in = params.land_in.clone();
            thread::spawn(move || project_path.open(Mutation::RotateLeft, land_in));
        }
        return Ok(Response::new(Body::from("")));
    }

    // /project/next - navigate to next project
    if rest == "next" {
        let projects = projects::lock();
        if let Some(project) = projects.next() {
            let project_path = project.as_project_path();
            let land_in = params.land_in.clone();
            thread::spawn(move || project_path.open(Mutation::RotateRight, land_in));
        }
        return Ok(Response::new(Body::from("")));
    }

    // Check for verb suffix: /project/<name>/remove or /project/<name>/close
    if let Some(name) = rest.strip_suffix("/remove") {
        if method != &Method::POST {
            return Ok(method_not_allowed("POST", &format!("/project/{}/remove", name)));
        }
        return Ok(endpoints::remove_project(name.trim()));
    }

    if let Some(name) = rest.strip_suffix("/close") {
        if method != &Method::POST {
            return Ok(method_not_allowed("POST", &format!("/project/{}/close", name)));
        }
        let name = name.trim().to_string();
        thread::spawn(move || endpoints::close_project(&name));
        return Ok(Response::new(Body::from("")));
    }

    // Default: /project/<name> - open project
    let name_or_path = rest.trim();
    let land_in = params.land_in.clone();
    let names = params.names.clone();

    if let Some((Some(project_path), mutation, land_in)) =
        open_project_by_name(name_or_path, land_in, names)
    {
        thread::spawn(move || project_path.open(mutation, land_in));
        Ok(Response::builder()
            .header("Content-Type", "text/html")
            .body(Body::from(
                "<html><body><script>window.close()</script>Sent into wormhole.</body></html>",
            ))
            .unwrap())
    } else {
        Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project not found: {}", name_or_path)))
            .unwrap())
    }
}

fn handle_task_request(
    method: &Method,
    rest: &str,
    params: &QueryParams,
) -> Result<Response<Body>, Infallible> {
    // Check for verb suffix: /task/<id>/delete
    if let Some(task_id) = rest.strip_suffix("/delete") {
        if method != &Method::POST {
            return Ok(method_not_allowed("POST", &format!("/task/{}/delete", task_id)));
        }
        let task_id = task_id.trim().to_string();
        return match crate::task::delete_task(&task_id) {
            Ok(()) => Ok(Response::new(Body::from(format!("Deleted task: {}", task_id)))),
            Err(e) => Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(e))
                .unwrap()),
        };
    }

    // Default: /task/<id> - open task
    let task_id = rest.trim().to_string();
    let home = params.home.clone();
    let land_in = params.land_in.clone();
    thread::spawn(move || {
        if let Err(e) = crate::task::open_task(&task_id, home.as_deref(), land_in) {
            crate::util::error(&e);
        }
    });
    Ok(Response::new(Body::from("")))
}

fn open_project_by_name(
    name_or_path: &str,
    land_in: Option<Application>,
    names: Vec<String>,
) -> Option<(Option<ProjectPath>, Mutation, Option<Application>)> {
    let mut projects = projects::lock();
    if let Some(project) = projects.by_name(name_or_path) {
        Some((Some(project.as_project_path()), Mutation::Insert, land_in))
    } else if name_or_path.starts_with('/') {
        let path = std::path::PathBuf::from(name_or_path);
        if let Some(project) = projects.by_exact_path(&path) {
            Some((Some(project.as_project_path()), Mutation::Insert, land_in))
        } else {
            projects.add(name_or_path, names);
            let project = projects.by_exact_path(&path);
            Some((
                project.map(|p| p.as_project_path()),
                Mutation::Insert,
                land_in,
            ))
        }
    } else {
        // Search WORMHOLE_PATH for a directory matching this name
        if let Some(path) = config::resolve_project_name(name_or_path) {
            let path_str = path.to_string_lossy().to_string();
            let mut project_names = names;
            if project_names.is_empty() {
                project_names = vec![name_or_path.to_string()];
            }
            projects.add(&path_str, project_names);
            let project = projects.by_exact_path(&path);
            Some((
                project.map(|p| p.as_project_path()),
                Mutation::Insert,
                land_in,
            ))
        } else {
            Some((None, Mutation::Insert, land_in))
        }
    }
}

fn determine_requested_operation(
    url_path: &str,
    line: Option<usize>,
    land_in: Option<Application>,
) -> Option<(Option<ProjectPath>, Mutation, Option<Application>)> {
    let projects = projects::lock();
    if let Some(absolute_path) = url_path.strip_prefix("/file/") {
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

async fn handle_kv_request(
    method: &Method,
    kv_path: &str,
    req: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    let parts: Vec<&str> = kv_path.split('/').collect();

    match parts.as_slice() {
        [project] if project.is_empty() => {
            // /kv/ - same as /kv
            Ok(crate::kv::get_all_kv())
        }
        [project] => {
            // /kv/<project> - get all KV for project
            if method == Method::GET {
                Ok(crate::kv::get_project_kv(project))
            } else {
                Ok(Response::builder()
                    .status(StatusCode::METHOD_NOT_ALLOWED)
                    .body(Body::from("Method not allowed. Use GET for /kv/<project>"))
                    .unwrap())
            }
        }
        [project, key] => {
            // /kv/<project>/<key>
            match *method {
                Method::GET => Ok(crate::kv::get_value(project, key)),
                Method::PUT => {
                    let (_, body) = req.into_parts();
                    Ok(crate::kv::set_value(project, key, body).await)
                }
                Method::DELETE => Ok(crate::kv::delete_value(project, key)),
                _ => Ok(Response::builder()
                    .status(StatusCode::METHOD_NOT_ALLOWED)
                    .body(Body::from("Method not allowed. Use GET, PUT, or DELETE"))
                    .unwrap()),
            }
        }
        _ => Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Invalid KV path format"))
            .unwrap()),
    }
}

impl QueryParams {
    pub fn from_query(query: Option<&str>) -> Self {
        let mut params = QueryParams {
            land_in: None,
            line: None,
            names: vec![],
            home: None,
        };
        if let Some(query) = query {
            for (key, val) in form_urlencoded::parse(query.as_bytes()).collect::<Vec<(_, _)>>() {
                let key_lower = key.to_lowercase();
                if key_lower == "land-in" {
                    let val_lower = val.to_lowercase();
                    if val_lower == "terminal" {
                        params.land_in = Some(Application::Terminal);
                    } else if val_lower == "editor" {
                        params.land_in = Some(Application::Editor);
                    }
                } else if key_lower == "line" {
                    params.line = val.parse::<usize>().ok();
                } else if key_lower == "name" {
                    params.names = val
                        .to_string()
                        .split(",")
                        .map(|s| s.trim().to_string())
                        .collect();
                } else if key_lower == "home" {
                    params.home = Some(val.to_string());
                }
            }
        }
        params
    }
}
