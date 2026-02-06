use std::convert::Infallible;
use std::thread;

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
    pub home_project: Option<String>,
    pub branch: Option<String>,
    pub skip_editor: bool,
    pub focus_terminal: bool,
    pub sync: bool,
    pub pwd: Option<String>,
    pub active: bool,
    pub current: Option<String>,
}

pub async fn service(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();

    if method == Method::OPTIONS {
        return Ok(cors_response(Response::new(Body::from(""))));
    }

    if path == "/favicon.ico" {
        return Ok(Response::new(Body::from("")));
    }

    let params = QueryParams::from_query(uri.query());
    let response = route(req, &method, &path, &params).await;
    if path != "/project/list" {
        ps!(
            "{} {} {} {:?}",
            response.status().as_u16(),
            method,
            uri,
            params
        );
    }
    Ok(response)
}

async fn route(
    req: Request<Body>,
    method: &Method,
    path: &str,
    params: &QueryParams,
) -> Response<Body> {
    match path {
        "/project/list" => endpoints::list_projects(params.active),
        "/project/neighbors" => endpoints::neighbors(params.active),
        "/project/debug" => endpoints::debug_projects(),
        "/dashboard" => endpoints::dashboard(),
        "/favicon.png" => endpoints::favicon(),
        "/shell" => endpoints::shell_env(params.pwd.as_deref()),
        "/project/previous" => {
            endpoints::navigate(endpoints::Direction::Previous, params);
            Response::new(Body::from(""))
        }
        "/project/next" => {
            endpoints::navigate(endpoints::Direction::Next, params);
            Response::new(Body::from(""))
        }
        "/project/pin" => require_post(method, || {
            thread::spawn(endpoints::pin_current);
            Response::new(Body::from("Pinning current state..."))
        }),
        "/project/show" => endpoints::show(None),
        "/project/current/poll" => {
            let wait = parse_prefer_wait(&req);
            endpoints::poll_current(params.current.as_deref(), wait).await
        }
        "/project/describe" => {
            require_post_async(method, || async { endpoints::describe(req).await }).await
        }
        "/project/refresh" => require_post(method, || {
            endpoints::refresh_all();
            Response::new(Body::from(""))
        }),
        "/project/refresh-tasks" => require_post(method, || {
            projects::refresh_tasks();
            Response::new(Body::from(""))
        }),
        "/kv" => crate::kv::list_all_kv_fresh(),
        _ => route_with_params(req, method, path, params).await,
    }
}

async fn route_with_params(
    req: Request<Body>,
    method: &Method,
    path: &str,
    params: &QueryParams,
) -> Response<Body> {
    if let Some(name) = path.strip_prefix("/project/remove/") {
        return require_post(method, || endpoints::remove(name));
    }
    if let Some(name) = path.strip_prefix("/project/close/") {
        return require_post(method, || {
            endpoints::close(name);
            Response::new(Body::from(""))
        });
    }
    if let Some(name) = path.strip_prefix("/project/show/") {
        return endpoints::show(Some(name.trim()));
    }
    if let Some(name) = path.strip_prefix("/project/refresh/") {
        return require_post(method, || endpoints::refresh_project(name));
    }
    if let Some(branch) = path.strip_prefix("/project/create/") {
        return endpoints::create_task(branch, params.home_project.as_deref());
    }
    if let Some(name) = path.strip_prefix("/project/switch/") {
        return cors_response(endpoints::switch(name, params, params.sync));
    }
    if let Some(name) = path.strip_prefix("/project/vscode/") {
        return cors_response(endpoints::vscode_url(name));
    }
    if let Some(kv_path) = path.strip_prefix("/kv/") {
        return handle_kv_request(method, kv_path, req).await;
    }

    route_file_or_github(path, params)
}

fn route_file_or_github(path: &str, params: &QueryParams) -> Response<Body> {
    if let Some((Some(project_path), mutation, land_in)) =
        determine_requested_operation(path, params.line, params.land_in.clone())
    {
        thread::spawn(move || project_path.open(mutation, land_in));
        Response::builder()
            .header("Content-Type", "text/html")
            .body(Body::from(endpoints::WORMHOLE_RESPONSE_HTML))
            .unwrap()
    } else {
        let redirect_to = format!(
            "https://github.com{path}#L{}?wormhole=false",
            params.line.unwrap_or(1)
        );
        ps!("Redirecting to: {}", redirect_to);
        Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, redirect_to)
            .body(Body::empty())
            .unwrap()
    }
}

fn determine_requested_operation(
    url_path: &str,
    line: Option<usize>,
    land_in: Option<Application>,
) -> Option<(Option<ProjectPath>, Mutation, Option<Application>)> {
    let projects = projects::lock();
    if let Some(absolute_path) = url_path.strip_prefix("/file/") {
        let p = ProjectPath::from_absolute_path(absolute_path, line, &projects);
        Some((p, Mutation::Insert, land_in))
    } else if let Some(project_path) = ProjectPath::from_github_url(url_path, line, &projects) {
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

fn require_post<F>(method: &Method, handler: F) -> Response<Body>
where
    F: FnOnce() -> Response<Body>,
{
    if *method == Method::POST {
        handler()
    } else {
        Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::from("Method not allowed. Use POST."))
            .unwrap()
    }
}

async fn require_post_async<F, Fut>(method: &Method, handler: F) -> Response<Body>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Response<Body>>,
{
    if *method == Method::POST {
        cors_response(handler().await)
    } else {
        cors_response(
            Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from("Use POST"))
                .unwrap(),
        )
    }
}

async fn handle_kv_request(method: &Method, kv_path: &str, req: Request<Body>) -> Response<Body> {
    use crate::kv;
    use crate::project::ProjectKey;

    let parts: Vec<&str> = kv_path.split('/').collect();

    match parts.as_slice() {
        [""] => kv::list_all_kv_fresh(),
        [project] => {
            if method == Method::GET {
                let key = ProjectKey::parse(project);
                kv::get_project_kv(&key)
            } else {
                Response::builder()
                    .status(StatusCode::METHOD_NOT_ALLOWED)
                    .body(Body::from("Method not allowed. Use GET for /kv/<project>"))
                    .unwrap()
            }
        }
        [project, kv_key] => {
            let key = ProjectKey::parse(project);
            match *method {
                Method::GET => kv::get_value(&key, kv_key),
                Method::PUT => {
                    let (_, body) = req.into_parts();
                    kv::set_value(&key, kv_key, body).await
                }
                Method::DELETE => kv::delete_value(&key, kv_key),
                _ => Response::builder()
                    .status(StatusCode::METHOD_NOT_ALLOWED)
                    .body(Body::from("Method not allowed. Use GET, PUT, or DELETE"))
                    .unwrap(),
            }
        }
        _ => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Invalid KV path format"))
            .unwrap(),
    }
}

impl QueryParams {
    pub fn from_query(query: Option<&str>) -> Self {
        let mut params = QueryParams {
            land_in: None,
            line: None,
            home_project: None,
            branch: None,
            skip_editor: false,
            focus_terminal: false,
            sync: false,
            pwd: None,
            active: false,
            current: None,
        };
        if let Some(query) = query {
            for (key, val) in form_urlencoded::parse(query.as_bytes()) {
                match key.to_lowercase().as_str() {
                    "land-in" => {
                        params.land_in = match val.to_lowercase().as_str() {
                            "terminal" => Some(Application::Terminal),
                            "editor" => Some(Application::Editor),
                            _ => None,
                        }
                    }
                    "line" => params.line = val.parse().ok(),
                    "home-project" => params.home_project = Some(val.to_string()),
                    "branch" => params.branch = Some(val.to_string()),
                    "skip-editor" => params.skip_editor = val.to_lowercase() == "true",
                    "focus-terminal" => params.focus_terminal = val.to_lowercase() == "true",
                    "sync" => params.sync = val == "true" || val == "1",
                    "pwd" => params.pwd = Some(val.to_string()),
                    "active" => params.active = val == "true" || val == "1",
                    "current" => {
                        params.current = if val.is_empty() {
                            None
                        } else {
                            Some(val.to_string())
                        }
                    }
                    _ => {}
                }
            }
        }
        params
    }
}

fn parse_prefer_wait(req: &Request<Body>) -> u64 {
    req.headers()
        .get("Prefer")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("wait="))
        .and_then(|s| s.parse().ok())
        .unwrap_or(30)
}

fn cors_response(response: Response<Body>) -> Response<Body> {
    let (mut parts, body) = response.into_parts();
    parts
        .headers
        .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_params_empty_current_is_none() {
        // Empty current= should be treated as None
        let params = QueryParams::from_query(Some("current="));
        assert!(
            params.current.is_none(),
            "Empty current= should be None, got {:?}",
            params.current
        );
    }

    #[test]
    fn test_query_params_missing_current_is_none() {
        // No current param should be None
        let params = QueryParams::from_query(Some("active=true"));
        assert!(params.current.is_none());
    }

    #[test]
    fn test_query_params_current_with_value() {
        // current=foo should be Some("foo")
        let params = QueryParams::from_query(Some("current=myproject"));
        assert_eq!(params.current, Some("myproject".to_string()));
    }
}
