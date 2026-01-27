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
    pub home_project: Option<String>,
    pub branch: Option<String>,
    pub format: Option<String>,
    pub skip_editor: bool,
    pub focus_terminal: bool,
    pub sync: bool,
}

pub async fn service(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let method = req.method().clone();
    let uri = req.uri();
    let path = uri.path().to_string();

    // Handle CORS preflight
    if method == Method::OPTIONS {
        return Ok(cors_response(Response::new(Body::from(""))));
    }

    if &path == "/favicon.ico" {
        return Ok(Response::new(Body::from("")));
    }
    let params = QueryParams::from_query(uri.query());
    if &path != "/project/list" {
        ps!("{} {} {:?}", method, uri, params);
    }
    if &path == "/project/list" {
        Ok(endpoints::list_projects())
    } else if &path == "/project/debug" {
        Ok(endpoints::debug_projects())
    } else if &path == "/api/sprint" {
        Ok(endpoints::sprint())
    } else if &path == "/dashboard" {
        Ok(endpoints::dashboard())
    } else if &path == "/project/previous" {
        let p = {
            let projects = projects::lock();
            projects.previous().map(|p| p.as_project_path())
        };
        if let Some(project_path) = p {
            let land_in = params.land_in.clone();
            thread::spawn(move || project_path.open(Mutation::RotateLeft, land_in));
        }
        Ok(Response::new(Body::from("")))
    } else if &path == "/project/next" {
        let p = {
            let projects = projects::lock();
            projects.next().map(|p| p.as_project_path())
        };
        if let Some(project_path) = p {
            let land_in = params.land_in.clone();
            thread::spawn(move || project_path.open(Mutation::RotateRight, land_in));
        }
        Ok(Response::new(Body::from("")))
    } else if let Some(name) = path.strip_prefix("/project/remove/") {
        if method != &Method::POST {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from(
                    "Method not allowed. Use POST for /project/remove/",
                ))
                .unwrap());
        }
        let name = name.trim();
        if let Some(task) = crate::task::get_task(name) {
            if task.home_project.is_some() {
                match crate::task::remove_task(name) {
                    Ok(()) => {
                        return Ok(Response::new(Body::from(format!("Removed task: {}", name))))
                    }
                    Err(e) => {
                        return Ok(Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::from(e))
                            .unwrap())
                    }
                }
            }
        }
        Ok(endpoints::remove_project(name))
    } else if let Some(name) = path.strip_prefix("/project/close/") {
        if method != &Method::POST {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from(
                    "Method not allowed. Use POST for /project/close/",
                ))
                .unwrap());
        }
        let name = name.trim().to_string();
        thread::spawn(move || endpoints::close_project(&name));
        Ok(Response::new(Body::from("")))
    } else if path == "/project/pin" {
        if method != &Method::POST {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from("Method not allowed. Use POST for /project/pin"))
                .unwrap());
        }
        thread::spawn(move || endpoints::pin_current());
        Ok(Response::new(Body::from("Pinning current state...")))
    } else if path == "/project/show" || path.starts_with("/project/show/") {
        let name = path.strip_prefix("/project/show/").map(|s| s.trim());
        let json_format = params.format.as_deref() == Some("json");
        let status = if let Some(n) = name.filter(|s| !s.is_empty()) {
            crate::status::get_status_by_name(n)
        } else {
            crate::status::get_current_status()
        };
        match status {
            Some(s) => {
                if json_format {
                    let json = serde_json::to_string_pretty(&s).unwrap_or_default();
                    Ok(Response::builder()
                        .header("Content-Type", "application/json")
                        .body(Body::from(json))
                        .unwrap())
                } else {
                    Ok(Response::new(Body::from(s.render_terminal())))
                }
            }
            None => Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Project not found"))
                .unwrap()),
        }
    } else if path == "/project/describe" {
        if method != &Method::POST {
            return Ok(cors_response(
                Response::builder()
                    .status(StatusCode::METHOD_NOT_ALLOWED)
                    .body(Body::from("Use POST"))
                    .unwrap(),
            ));
        }
        let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();
        let request: Result<crate::describe::DescribeRequest, _> =
            serde_json::from_slice(&body_bytes);
        match request {
            Ok(req) => {
                let response = crate::describe::describe(&req);
                let json = serde_json::to_string_pretty(&response).unwrap_or_default();
                Ok(cors_response(
                    Response::builder()
                        .header("Content-Type", "application/json")
                        .body(Body::from(json))
                        .unwrap(),
                ))
            }
            Err(e) => Ok(cors_response(
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!("Invalid JSON: {}", e)))
                    .unwrap(),
            )),
        }
    } else if let Some(name) = path.strip_prefix("/project/refresh/") {
        if method != &Method::POST {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from("Use POST"))
                .unwrap());
        }
        let name = name.trim();
        let mut projects = projects::lock();
        if let Some(project) = projects.get_mut(name) {
            crate::github::refresh_github_info(project);
            let json = serde_json::json!({
                "name": project.name,
                "github_pr": project.github_pr,
                "github_repo": project.github_repo,
            });
            Ok(Response::builder()
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_string_pretty(&json).unwrap()))
                .unwrap())
        } else {
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!("Project '{}' not found", name)))
                .unwrap())
        }
    } else if let Some(task_id) = path.strip_prefix("/project/create/") {
        let task_id = task_id.trim();
        let home = match params.home_project.as_deref() {
            Some(h) => h,
            None => {
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from("home-project query param required"))
                    .unwrap())
            }
        };
        match crate::task::create_task(task_id, home, params.branch.as_deref()) {
            Ok(_) => Ok(Response::new(Body::from(format!(
                "Created task: {}",
                task_id
            )))),
            Err(e) => Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(e))
                .unwrap()),
        }
    } else if let Some(name_or_path) = path.strip_prefix("/project/switch/") {
        let name_or_path = name_or_path.trim().to_string();
        let home_project = params.home_project.clone();
        let branch = params.branch.clone();
        let land_in = params.land_in.clone();
        let names = params.names.clone();
        let skip_editor = params.skip_editor;
        let focus_terminal = params.focus_terminal;
        let do_switch = move || -> Result<(), String> {
            if home_project.is_some() || crate::task::get_task(&name_or_path).is_some() {
                crate::task::open_task(
                    &name_or_path,
                    home_project.as_deref(),
                    branch.as_deref(),
                    land_in,
                    skip_editor,
                    focus_terminal,
                )
            } else {
                let project_path = {
                    let mut projects = projects::lock();
                    resolve_project(&mut projects, &name_or_path, names)
                };
                if let Some(pp) = project_path {
                    pp.open(Mutation::Insert, land_in);
                }
                Ok(())
            }
        };
        if params.sync {
            match do_switch() {
                Ok(()) => Ok(cors_response(Response::new(Body::from("ok")))),
                Err(e) => Ok(cors_response(
                    Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from(e))
                        .unwrap(),
                )),
            }
        } else {
            thread::spawn(move || {
                if let Err(e) = do_switch() {
                    crate::util::error(&e);
                }
            });
            Ok(cors_response(
                Response::builder()
                    .header("Content-Type", "text/html")
                    .body(Body::from(
                        "<html><body><script>window.close()</script>Sent into wormhole.</body></html>",
                    ))
                    .unwrap(),
            ))
        }
    } else if path == "/kv" {
        Ok(crate::kv::get_all_kv())
    } else if let Some(kv_path) = path.strip_prefix("/kv/") {
        handle_kv_request(&method, kv_path, req).await
    } else {
        // Handle /file/ and GitHub blob URLs
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

fn resolve_project(
    projects: &mut projects::Projects,
    name_or_path: &str,
    names: Vec<String>,
) -> Option<ProjectPath> {
    if let Some(project) = projects.by_name(name_or_path) {
        Some(project.as_project_path())
    } else if name_or_path.starts_with('/') {
        let path = std::path::PathBuf::from(name_or_path);
        if let Some(project) = projects.by_exact_path(&path) {
            Some(project.as_project_path())
        } else {
            projects.add(name_or_path, names);
            projects.by_exact_path(&path).map(|p| p.as_project_path())
        }
    } else if let Some(path) = config::resolve_project_name(name_or_path) {
        let path_str = path.to_string_lossy().to_string();
        let mut project_names = names;
        if project_names.is_empty() {
            project_names = vec![name_or_path.to_string()];
        }
        projects.add(&path_str, project_names);
        projects.by_exact_path(&path).map(|p| p.as_project_path())
    } else {
        None
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
            home_project: None,
            branch: None,
            format: None,
            skip_editor: false,
            focus_terminal: false,
            sync: false,
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
                } else if key_lower == "home-project" {
                    params.home_project = Some(val.to_string());
                } else if key_lower == "branch" {
                    params.branch = Some(val.to_string());
                } else if key_lower == "format" {
                    params.format = Some(val.to_string());
                } else if key_lower == "skip-editor" {
                    params.skip_editor = val.to_lowercase() == "true";
                } else if key_lower == "focus-terminal" {
                    params.focus_terminal = val.to_lowercase() == "true";
                } else if key_lower == "sync" {
                    params.sync = val == "true" || val == "1";
                }
            }
        }
        params
    }
}

fn cors_response(response: Response<Body>) -> Response<Body> {
    let (mut parts, body) = response.into_parts();
    parts.headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().unwrap(),
    );
    parts.headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        "GET, POST, OPTIONS".parse().unwrap(),
    );
    parts.headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        "Content-Type".parse().unwrap(),
    );
    Response::from_parts(parts, body)
}
