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
use std::{fs, io};

use hyper::server::conn::AddrIncoming;
use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
use hyper_rustls::TlsAcceptor;
use rustls_pemfile;

use util::{error, warn};

#[tokio::main]
async fn main() {
    project::read_projects();
    tokio::join!(serve_http(), serve_https());
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

async fn serve_https() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 443));
    let incoming = AddrIncoming::bind(&addr).unwrap();

    let certs = load_certs("/Users/dan/src/wormhole/cert/cert.pem").unwrap();
    let key = load_private_key("/Users/dan/src/wormhole/cert/key.pem").unwrap();
    let acceptor = TlsAcceptor::builder()
        .with_single_cert(certs, key)
        .map_err(|e| error(&format!("{}", e)))
        .unwrap()
        .with_all_versions_alpn()
        .with_incoming(incoming);

    let make_service =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(wormhole::service)) });

    // Serve forever: a Wormhole service is created for each incoming connection
    let server = Server::builder(acceptor).serve(make_service);

    if let Err(e) = server.await {
        warn(&format!("server error: {}", e));
    }
}

// Load public certificate from file.
fn load_certs(filename: &str) -> io::Result<Vec<rustls::Certificate>> {
    // Open certificate file.
    let certfile = fs::File::open(filename).unwrap();
    let mut reader = io::BufReader::new(certfile);

    // Load and return certificate.
    let certs = rustls_pemfile::certs(&mut reader).unwrap();
    Ok(certs.into_iter().map(rustls::Certificate).collect())
}

// Load private key from file.
fn load_private_key(filename: &str) -> io::Result<rustls::PrivateKey> {
    // Open keyfile.
    let keyfile = fs::File::open(filename).unwrap();
    let mut reader = io::BufReader::new(keyfile);

    // Load and return a single private key.
    let keys = rustls_pemfile::pkcs8_private_keys(&mut reader).unwrap();
    if keys.len() != 1 {
        error("expected a single private key");
    }

    Ok(rustls::PrivateKey(keys[0].clone()))
}
