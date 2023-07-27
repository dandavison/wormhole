mod endpoints;
mod handlers;
mod project;
mod project_path;
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
        Ok(endpoints::projects())
    } else {
        let _ = handlers::open_path_in_vscode(&path).unwrap()
            || handlers::open_project_in_vscode(&path).unwrap()
            || handlers::open_github_url_in_vscode(&path).unwrap();
        Ok(Response::new(Body::from("Sent to wormhole.")))
    }
}

#[tokio::main]
async fn main() {
    project::PROJECTS.get_or_init(project::read_projects);
    let addr = SocketAddr::from(([127, 0, 0, 1], 80));

    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(wormhole)) });

    // Serve forever: a Wormhole service is created for each incoming connection
    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
