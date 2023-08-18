mod config;
mod editor;
mod endpoints;
mod hammerspoon;
mod handlers;
mod project;
mod project_path;
mod terminal;
mod tmux;
mod util;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::thread;

// use hyper::server::conn::AddrIncoming;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
// use hyper_rustls::TlsAcceptor;
use url::form_urlencoded;

use util::warn;

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

pub struct QueryParams {
    pub land_in: Option<Application>,
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
                    if val == "terminal" {
                        params.land_in = Some(Application::Terminal);
                    } else if val == "editor" {
                        params.land_in = Some(Application::Editor);
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
    tokio::join!(serve_http());
}

async fn serve_http() {
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

// async fn serve_https() {
//     let addr = SocketAddr::from(([127, 0, 0, 1], 443));
//     let incoming = AddrIncoming::bind(&addr).unwrap();

//     let certs = load_certs();
//     let key = load_key();
//     let acceptor = TlsAcceptor::builder()
//         .with_single_cert(certs, key)
//         .map_err(|e| error(format!("{}", e)))
//         .with_all_versions_alpn() // wtf?
//         .with_incoming(incoming);

//     let make_svc = make_service_fn(|_conn| async {
//         println!("hello");
//         Ok::<_, Infallible>(service_fn(wormhole_spawner))
//     });

//     // Serve forever: a Wormhole service is created for each incoming connection
//     let server = Server::builder(acceptor).serve(make_svc);

//     if let Err(e) = server.await {
//         warn(&format!("server error: {}", e));
//     }
// }
