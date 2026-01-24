mod cli;
mod config;
mod editor;
mod endpoints;
mod git;
mod hammerspoon;
mod jira;
mod kv;
mod project;
mod project_path;
mod projects;
mod task;
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
use std::process;

use clap::Parser;
use hyper::service::{make_service_fn, service_fn};
use hyper::Server;

use cli::{Cli, Command};
use util::warn;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        // No subcommand or explicit "serve" -> start server
        None | Some(Command::Serve) => {
            projects::load();
            serve_http().await;
        }
        // Other subcommands -> run as client
        Some(cmd) => {
            if let Err(e) = cli::run(cmd) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
    }
}

async fn serve_http() {
    let addr = SocketAddr::from(([127, 0, 0, 1], config::wormhole_port()));

    let make_service =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(wormhole::service)) });

    // Serve forever: a Wormhole service is created for each incoming connection
    let server = Server::bind(&addr).serve(make_service);

    if let Err(e) = server.await {
        warn(&format!("server error: {}", e));
    }
}
