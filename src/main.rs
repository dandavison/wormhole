mod config;
mod editor;
mod endpoints;
mod hammerspoon;
mod project;
mod project_path;
mod terminal;
mod tmux;
mod util;
mod wormhole;

use std::convert::Infallible;
use std::net::SocketAddr;

// use hyper::server::conn::AddrIncoming;
use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
// use hyper_rustls::TlsAcceptor;

use util::warn;

#[tokio::main]
async fn main() {
    project::read_projects();
    tokio::join!(serve_http());
}

async fn serve_http() {
    project::read_projects();
    let addr = SocketAddr::from(([127, 0, 0, 1], 80));

    let make_service =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(wormhole::service)) });

    // Serve forever: a Wormhole service is created for each incoming connection
    let server = Server::bind(&addr).serve(make_service);

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

//     let make_service = make_service_fn(|_conn| async {
//         println!("hello");
//         Ok::<_, Infallible>(service_fn(wormhole_spawner))
//     });

//     // Serve forever: a Wormhole service is created for each incoming connection
//     let server = Server::builder(acceptor).serve(make_service);

//     if let Err(e) = server.await {
//         warn(&format!("server error: {}", e));
//     }
// }
