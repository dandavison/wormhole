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

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use util::warn;

async fn wormhole(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let uri = req.uri();
    println!("Request: {}", uri);
    let path = uri.path().to_string();
    if &path == "/favicon.ico" {
        return Ok(Response::new(Body::from(
            "Stop sending me /favicon requests",
        )));
    }
    let sent_into_wormhole = Response::new(Body::from("Sent into wormhole."));
    if &path == "/list-projects/" {
        Ok(endpoints::list_projects())
    } else if let Some(path) = path.strip_prefix("/add-project/") {
        // An absolute path must have a double slash: /add-project//Users/me/file.rs
        Ok(endpoints::add_project(&path))
    } else if let Some(name) = path.strip_prefix("/remove-project/") {
        Ok(endpoints::remove_project(&name))
    } else if let Some(name) = path.strip_prefix("/project/") {
        handlers::select_project_by_name(name, uri.query());
        Ok(sent_into_wormhole)
    } else if let Some(absolute_path) = path.strip_prefix("/file/") {
        handlers::select_project_by_path(absolute_path);
        Ok(sent_into_wormhole)
    } else {
        handlers::select_project_by_github_url(&path, uri.query()).unwrap();
        Ok(sent_into_wormhole)
    }
}

#[tokio::main]
async fn main() {
    project::read_projects();
    let addr = SocketAddr::from(([127, 0, 0, 1], 80));

    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(wormhole)) });

    // Serve forever: a Wormhole service is created for each incoming connection
    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        warn(&format!("server error: {}", e));
    }
}
