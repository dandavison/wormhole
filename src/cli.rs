use clap::builder::ValueHint;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use std::io;

use crate::config;

#[derive(Parser)]
#[command(name = "wormhole")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Switch to a project by name or path
    Switch {
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
    /// List projects
    List {
        /// Output format: text (default) or json
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Switch to previous project
    Previous {
        /// Which application to focus: editor or terminal
        #[arg(long, value_name = "APP")]
        land_in: Option<String>,
    },
    /// Switch to next project
    Next {
        /// Which application to focus: editor or terminal
        #[arg(long, value_name = "APP")]
        land_in: Option<String>,
    },
    /// Close project windows
    Close {
        /// Project name
        name: String,
    },
    /// Remove project from wormhole
    Remove {
        /// Project name
        name: String,
    },
}

#[derive(Subcommand)]
pub enum TaskCommand {
    /// Switch to a task
    Switch {
        /// Task identifier
        task_id: String,
        /// Home repository name (required for new tasks)
        #[arg(long)]
        home: Option<String>,
        /// Which application to focus: editor or terminal
        #[arg(long, value_name = "APP")]
        land_in: Option<String>,
    },
    /// List tasks
    List,
    /// Delete task (remove worktree and branch)
    Delete {
        /// Task identifier
        task_id: String,
    },
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the wormhole server
    Serve,

    /// Project operations
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },

    /// Task operations
    Task {
        #[command(subcommand)]
        command: TaskCommand,
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

    /// Pin current (project, application) state
    Pin,

    /// Key-value storage operations
    Kv {
        #[command(subcommand)]
        command: KvCommand,
    },

    /// Show debug information
    Debug,

    /// Generate shell completions
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
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

        Command::Project { command } => match command {
            ProjectCommand::Switch {
                name_or_path,
                name,
                land_in,
            } => {
                let query = build_query(&land_in, &name);
                client.get(&format!("/project/switch/{}{}", name_or_path, query))?;
                Ok(())
            }
            ProjectCommand::List { format } => {
                let response = client.get("/project/list")?;
                if format == "json" {
                    println!("{}", response);
                } else {
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
            ProjectCommand::Previous { land_in } => {
                let query = build_query(&land_in, &None);
                client.get(&format!("/project/previous{}", query))?;
                Ok(())
            }
            ProjectCommand::Next { land_in } => {
                let query = build_query(&land_in, &None);
                client.get(&format!("/project/next{}", query))?;
                Ok(())
            }
            ProjectCommand::Close { name } => {
                client.post(&format!("/project/close/{}", name))?;
                Ok(())
            }
            ProjectCommand::Remove { name } => {
                client.post(&format!("/project/remove/{}", name))?;
                Ok(())
            }
        },

        Command::Task { command } => match command {
            TaskCommand::Switch {
                task_id,
                home,
                land_in,
            } => {
                let mut params = vec![];
                if let Some(h) = home {
                    params.push(format!("home={}", h));
                }
                if let Some(app) = land_in {
                    params.push(format!("land-in={}", app));
                }
                let query = if params.is_empty() {
                    String::new()
                } else {
                    format!("?{}", params.join("&"))
                };
                client.get(&format!("/task/switch/{}{}", task_id, query))?;
                Ok(())
            }
            TaskCommand::List => {
                let response = client.get("/task/list")?;
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                    if let Some(tasks) = json.get("tasks").and_then(|v| v.as_array()) {
                        for task in tasks {
                            if let Some(id) = task.get("id").and_then(|v| v.as_str()) {
                                println!("{}", id);
                            }
                        }
                    }
                }
                Ok(())
            }
            TaskCommand::Delete { task_id } => {
                client.post(&format!("/task/delete/{}", task_id))?;
                Ok(())
            }
        },

        Command::File { path, land_in } => {
            let query = build_query(&land_in, &None);
            let url_path = format!("/file/{}{}", path, query);
            client.get(&url_path)?;
            Ok(())
        }

        Command::Pin => {
            client.post("/pin")?;
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
            let response = client.get("/debug")?;
            println!("{}", response);
            Ok(())
        }

        Command::Completion { shell } => {
            generate(shell, &mut Cli::command(), "wormhole", &mut io::stdout());
            Ok(())
        }
    }
}
