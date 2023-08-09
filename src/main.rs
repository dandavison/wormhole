mod config;
mod endpoints;
mod hammerspoon;
mod handlers;
mod project;
mod project_path;
mod tmux;
mod util;
mod vscode;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::thread;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use url::form_urlencoded;

use util::warn;

pub enum Destination {
    VSCode,
    Tmux,
}

pub enum WindowAction {
    Focus,
    Raise,
}

pub struct QueryParams {
    pub land_in: Option<Destination>,
    pub line: Option<usize>,
}

async fn wormhole_spawner(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let uri = req.uri();
    println!("\nRequest: {}", uri);
    let path = uri.path().to_string();
    if &path == "/favicon.ico" {
        return Ok(Response::new(Body::from("")));
    }
    let params = QueryParams::from_query(uri.query());
    if &path == "/list-projects/" {
        Ok(endpoints::list_projects())
    } else if let Some(path) = path.strip_prefix("/add-project/") {
        // An absolute path must have a double slash: /add-project//Users/me/file.rs
        Ok(endpoints::add_project(&path))
    } else if let Some(name) = path.strip_prefix("/remove-project/") {
        Ok(endpoints::remove_project(&name))
    } else {
        thread::spawn(|| wormhole(path, params));
        Ok(Response::new(Body::from("Sent into wormhole.")))
    }
}

fn wormhole(path: String, params: QueryParams) {
    if path == "/previous-project/" {
        if let Some(project) = project::previous_project() {
            handlers::select_project_by_name(&project.name, None);
        } else {
            warn("There is no previous project");
        }
    } else if let Some(name) = path.strip_prefix("/project/") {
        handlers::select_project_by_name(name, params.land_in);
    } else if let Some(absolute_path) = path.strip_prefix("/file/") {
        handlers::select_project_by_path(absolute_path, params.land_in);
    } else {
        handlers::select_project_by_github_url(&path, params.line, params.land_in).unwrap();
    };
}

impl QueryParams {
    pub fn from_query(query: Option<&str>) -> Self {
        let mut params = QueryParams {
            land_in: None,
            line: None,
        };
        if let Some(query) = query {
            for (key, val) in
                form_urlencoded::parse(query.to_lowercase().as_bytes()).collect::<Vec<(_, _)>>()
            {
                if key == "land-in" {
                    if val == "tmux" {
                        params.land_in = Some(Destination::Tmux);
                    } else if val == "vscode" {
                        params.land_in = Some(Destination::VSCode);
                    }
                } else if key == "line" {
                    params.line = val.parse::<usize>().ok();
                }
            }
        }
        params
    }
}

#[tokio::main]
async fn main() {
    project::read_projects();
    let addr = SocketAddr::from(([127, 0, 0, 1], 80));

    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(wormhole_spawner)) });

    // Serve forever: a Wormhole service is created for each incoming connection
    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        warn(&format!("server error: {}", e));
    }
}
