mod config;
mod endpoints;
mod hammerspoon;
mod handlers;
mod project;
mod project_path;
mod tmux;
mod vscode;

use std::convert::Infallible;
use std::net::SocketAddr;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};

async fn wormhole(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let path = req.uri().to_string();
    if &path == "/favicon.ico" {
        return Ok(Response::new(Body::from(
            "Stop sending me /favicon requests",
        )));
    }
    println!("Request: {}", &path);
    if &path == "/list-projects/" {
        Ok(endpoints::list_projects())
    } else if path.starts_with("/add-project/") {
        Ok(endpoints::add_project(&path))
    } else {
        let _ = handlers::select_project_by_path(&path).unwrap()
            || handlers::select_project_by_name(&path).unwrap()
            || handlers::select_project_by_github_url(&path).unwrap();
        Ok(Response::new(Body::from("Sent to wormhole.")))
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
        eprintln!("server error: {}", e);
    }
}
