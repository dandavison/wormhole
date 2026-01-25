use clap::builder::ValueHint;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use std::io;

use crate::config;
use crate::jira;

#[derive(Parser)]
#[command(name = "wormhole")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum JiraCommand {
    /// List current sprint issues
    Sprint,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the wormhole server
    Serve,

    /// Switch to a project by name, or open/create a project at a path
    Project {
        /// Project name or absolute path
        #[arg(value_hint = ValueHint::DirPath)]
        name_or_path: String,
        /// Optional project name (when creating from path)
        #[arg(long)]
        name: Option<String>,
        /// Which application to focus: editor or terminal
        #[arg(long, value_name = "APP")]
        land_in: Option<String>,
    },

    /// Open a file in the appropriate project
    File {
        /// Absolute file path (optionally with :line suffix)
        #[arg(value_hint = ValueHint::FilePath)]
        path: String,
        /// Which application to focus: editor or terminal
        #[arg(long, value_name = "APP")]
        land_in: Option<String>,
    },

    /// Switch to the previous project
    Previous {
        /// Which application to focus: editor or terminal
        #[arg(long, value_name = "APP")]
        land_in: Option<String>,
    },

    /// Switch to the next project
    Next {
        /// Which application to focus: editor or terminal
        #[arg(long, value_name = "APP")]
        land_in: Option<String>,
    },

    /// List projects (current and available)
    List {
        /// Output format: text (default) or json
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Close a project (editor and terminal windows)
    Close {
        /// Project name
        name: String,
    },

    /// Remove a project from wormhole
    Remove {
        /// Project name
        name: String,
    },

    /// Pin current (project, application) state
    Pin,

    /// Key-value storage operations
    Kv {
        #[command(subcommand)]
        command: KvCommand,
    },

    /// Show debug information about all projects
    Debug,

    /// JIRA operations
    Jira {
        #[command(subcommand)]
        command: JiraCommand,
    },

    /// Generate shell completions
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Kill tmux session and clean up
    KillSession,
}

#[derive(Subcommand)]
pub enum KvCommand {
    /// Get a value
    Get {
        /// Project name
        project: String,
        /// Key name
        key: String,
    },
    /// Set a value
    Set {
        /// Project name
        project: String,
        /// Key name
        key: String,
        /// Value to set
        value: String,
    },
    /// Delete a key
    Delete {
        /// Project name
        project: String,
        /// Key name
        key: String,
    },
    /// List all KV pairs for a project
    List {
        /// Project name (optional, lists all if omitted)
        project: Option<String>,
    },
}

/// HTTP client for communicating with the wormhole server
pub struct Client {
    base_url: String,
}

impl Client {
    pub fn new() -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{}", config::wormhole_port()),
        }
    }

    fn get(&self, path: &str) -> Result<String, String> {
        ureq::get(&format!("{}{}", self.base_url, path))
            .call()
            .map_err(|e| format!("Request failed: {}", e))?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    fn post(&self, path: &str) -> Result<String, String> {
        ureq::post(&format!("{}{}", self.base_url, path))
            .call()
            .map_err(|e| format!("Request failed: {}", e))?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    fn put(&self, path: &str, body: &str) -> Result<String, String> {
        ureq::put(&format!("{}{}", self.base_url, path))
            .send_string(body)
            .map_err(|e| format!("Request failed: {}", e))?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    fn delete(&self, path: &str) -> Result<String, String> {
        ureq::delete(&format!("{}{}", self.base_url, path))
            .call()
            .map_err(|e| format!("Request failed: {}", e))?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }
}

fn build_query(land_in: &Option<String>, name: &Option<String>) -> String {
    let mut params = vec![];
    if let Some(app) = land_in {
        params.push(format!("land-in={}", app));
    }
    if let Some(n) = name {
        params.push(format!("name={}", n));
    }
    if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    }
}

pub fn run(command: Command) -> Result<(), String> {
    let client = Client::new();

    match command {
        Command::Serve => {
            // This is handled specially in main.rs
            unreachable!("Serve command should be handled in main")
        }

        Command::Project {
            name_or_path,
            name,
            land_in,
        } => {
            let query = build_query(&land_in, &name);
            let path = format!("/project/{}{}", name_or_path, query);
            client.get(&path)?;
            Ok(())
        }

        Command::File { path, land_in } => {
            let query = build_query(&land_in, &None);
            let url_path = format!("/file/{}{}", path, query);
            client.get(&url_path)?;
            Ok(())
        }

        Command::Previous { land_in } => {
            let query = build_query(&land_in, &None);
            client.get(&format!("/previous-project/{}", query))?;
            Ok(())
        }

        Command::Next { land_in } => {
            let query = build_query(&land_in, &None);
            client.get(&format!("/next-project/{}", query))?;
            Ok(())
        }

        Command::List { format } => {
            let response = client.get("/list-projects/")?;
            if format == "json" {
                println!("{}", response);
            } else {
                // Parse JSON and print current projects as plain text
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                    if let Some(current) = json.get("current").and_then(|v| v.as_array()) {
                        for name in current {
                            if let Some(s) = name.as_str() {
                                println!("{}", s);
                            }
                        }
                    }
                }
            }
            Ok(())
        }

        Command::Close { name } => {
            client.post(&format!("/close-project/{}", name))?;
            Ok(())
        }

        Command::Remove { name } => {
            client.post(&format!("/remove-project/{}", name))?;
            Ok(())
        }

        Command::Pin => {
            client.post("/pin/")?;
            Ok(())
        }

        Command::Kv { command } => match command {
            KvCommand::Get { project, key } => {
                let response = client.get(&format!("/kv/{}/{}", project, key))?;
                println!("{}", response);
                Ok(())
            }
            KvCommand::Set {
                project,
                key,
                value,
            } => {
                client.put(&format!("/kv/{}/{}", project, key), &value)?;
                Ok(())
            }
            KvCommand::Delete { project, key } => {
                client.delete(&format!("/kv/{}/{}", project, key))?;
                Ok(())
            }
            KvCommand::List { project } => {
                let path = match project {
                    Some(p) => format!("/kv/{}", p),
                    None => "/kv".to_string(),
                };
                let response = client.get(&path)?;
                println!("{}", response);
                Ok(())
            }
        },

        Command::Debug => {
            let response = client.get("/debug-projects/")?;
            println!("{}", response);
            Ok(())
        }

        Command::Jira { command } => match command {
            JiraCommand::Sprint => jira::print_sprint_issues(),
        },

        Command::Completion { shell } => {
            generate(shell, &mut Cli::command(), "wormhole", &mut io::stdout());
            Ok(())
        }

        Command::KillSession => {
            let _ = std::fs::remove_file("/tmp/wormhole.env");
            std::process::Command::new("tmux")
                .args(["kill-session"])
                .status()
                .map_err(|e| format!("Failed to kill tmux session: {}", e))?;
            Ok(())
        }
    }
}
