mod config;
mod editor;
mod endpoints;
mod hammerspoon;
mod project;
mod project_path;
mod projects;
mod terminal;
mod tmux;
mod util;
mod wezterm;
mod wormhole;
#[macro_use]
pub mod pst;
pub use pst::*;

use std::convert::Infallible;
use std::net::SocketAddr;

// use hyper::server::conn::AddrIncoming;
use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
// use hyper_rustls::TlsAcceptor;

use util::warn;

#[tokio::main]
async fn main() {
    projects::load();
    tokio::join!(serve_http());
}

async fn serve_http() {
    let addr = SocketAddr::from(([127, 0, 0, 1], config::WORMHOLE_PORT));

    let make_service =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(wormhole::service)) });

    // Serve forever: a Wormhole service is created for each incoming connection
    let server = Server::bind(&addr).serve(make_service);

    if let Err(e) = server.await {
        warn(&format!("server error: {}", e));
    }
}
