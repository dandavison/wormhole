mod cli;
mod config;
mod describe;
mod editor;
mod endpoints;
mod git;
mod github;
mod hammerspoon;
mod jira;
mod kv;
mod project;
mod project_path;
mod projects;
mod serve_web;
mod status;
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

use cli::{Cli, Command, ServerCommand};
use util::warn;

const SUBCOMMANDS: &[&str] = &[
    "server",
    "project",
    "open",
    "kv",
    "jira",
    "task",
    "completion",
    "kill",
    "doctor",
    "refresh",
];

fn fallback_to_open() -> Option<Command> {
    let args: Vec<String> = std::env::args().collect();
    let first_arg = args.get(1)?;
    if first_arg.starts_with('-') || SUBCOMMANDS.contains(&first_arg.as_str()) {
        return None;
    }
    Some(Command::Open {
        target: first_arg.clone(),
        land_in: None,
    })
}

#[tokio::main]
async fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // If parsing failed, check if first arg is an existing path
            if let Some(cmd) = fallback_to_open() {
                Cli { command: Some(cmd) }
            } else {
                e.exit();
            }
        }
    };

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
    let addr = SocketAddr::from(([127, 0, 0, 1], config::wormhole_port()));

    let make_service =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(wormhole::service)) });

    // Serve forever: a Wormhole service is created for each incoming connection
    let server = Server::bind(&addr).serve(make_service);

    if let Err(e) = server.await {
        warn(&format!("server error: {}", e));
    }
}
