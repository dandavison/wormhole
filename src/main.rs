mod handlers;
mod repo_paths;
mod tower_hyper_tonic;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::convert::Infallible;
use std::net::SocketAddr;

async fn wormhole(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let repo_paths = repo_paths::repo_paths();
    let path = req.uri().to_string();
    println!("Request: {}", &path);
    let _ = handlers::open_file(&path, &repo_paths).unwrap()
        || handlers::open_github_url(&path, &repo_paths).unwrap();
    Ok(Response::new(Body::from("Sent to wormhole.")))
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 2], 80));

    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(wormhole)) });

    // Serve forever: a Wormhole service is created for each incoming connection
    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
