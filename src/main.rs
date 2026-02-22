mod agent;
mod batch;
mod cli;
mod config;
mod editor;
mod git;
mod github;
mod hammerspoon;
mod handlers;
mod jira;
mod kv;
mod messages;
mod project;
mod project_path;
mod projects;
mod prompts;
mod serve_web;
mod status;
mod task;
mod terminal;
mod tmux;
mod util;
mod wezterm;
mod wormhole;
#[macro_use]
pub mod tty;
pub use tty::*;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::process;

use clap::{CommandFactory, Parser};
use clap_complete::env::CompleteEnv;
use hyper::service::{make_service_fn, service_fn};
use hyper::Server;

use cli::{Cli, Command, ServerCommand};
use util::warn;

#[tokio::main]
async fn main() {
    CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();

    match cli.command {
        // No subcommand or "server start-foreground" -> start server
        None
        | Some(Command::Server {
            command: ServerCommand::StartForeground,
        }) => {
            projects::load();
            // Refresh cache in background so server starts immediately
            std::thread::spawn(projects::refresh_cache);
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
    let port = config::wormhole_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let make_service =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(wormhole::service)) });

    let server = match Server::try_bind(&addr) {
        Ok(builder) => builder.serve(make_service),
        Err(e) => {
            eprintln!("Error: cannot bind to port {port}: {e}. Is another wormhole server already running?");
            process::exit(1);
        }
    };

    if let Err(e) = server.await {
        warn(&format!("server error: {}", e));
    }
}
