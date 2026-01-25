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
    /// Sprint operations
    Sprint {
        #[command(subcommand)]
        command: Option<SprintCommand>,
    },
}

#[derive(Subcommand)]
pub enum SprintCommand {
    /// Create tasks for sprint issues
    Create {
        /// Override pairs: <ticket> <home-project> ...
        /// Tickets not listed use WORMHOLE_DEFAULT_HOME_PROJECT
        #[arg(trailing_var_arg = true)]
        overrides: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Switch to a project by name, or open/create a project at a path
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
        /// Home project for creating a task (git worktree)
        #[arg(long)]
        home_project: Option<String>,
    },
    /// List projects (current and available)
    List {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
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
    /// Close a project (editor and terminal windows)
    Close {
        /// Project name
        name: String,
    },
    /// Remove a project from wormhole (removes worktree for tasks)
    Remove {
        /// Project name
        name: String,
    },
    /// Pin current (project, application) state
    Pin,
    /// Show debug information about all projects
    Debug,
    /// Show status of a project/task (JIRA, PR, etc.)
    Status {
        /// Project name (defaults to current project)
        name: Option<String>,
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
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

    /// Open a file in the appropriate project
    File {
        /// Absolute file path (optionally with :line suffix)
        #[arg(value_hint = ValueHint::FilePath)]
        path: String,
        /// Which application to focus: editor or terminal
        #[arg(long, value_name = "APP")]
        land_in: Option<String>,
    },

    /// Key-value storage operations
    Kv {
        #[command(subcommand)]
        command: KvCommand,
    },

    /// JIRA operations
    Jira {
        #[command(subcommand)]
        command: JiraCommand,
    },

    /// Generate shell completions
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum, required_unless_present = "projects")]
        shell: Option<Shell>,
        /// Output project names (for dynamic completion)
        #[arg(long)]
        projects: bool,
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

fn build_switch_query(
    land_in: &Option<String>,
    name: &Option<String>,
    home_project: &Option<String>,
) -> String {
    let mut params = vec![];
    if let Some(app) = land_in {
        params.push(format!("land-in={}", app));
    }
    if let Some(n) = name {
        params.push(format!("name={}", n));
    }
    if let Some(h) = home_project {
        params.push(format!("home-project={}", h));
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
            unreachable!("Serve command should be handled in main")
        }

        Command::Project { command } => match command {
            ProjectCommand::Switch {
                name_or_path,
                name,
                land_in,
                home_project,
            } => {
                let query = build_switch_query(&land_in, &name, &home_project);
                let path = format!("/project/switch/{}{}", name_or_path, query);
                client.get(&path)?;
                Ok(())
            }
            ProjectCommand::List { output } => {
                let response = client.get("/project/list")?;
                if output == "json" {
                    println!("{}", response);
                } else {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                        if let Some(current) = json.get("current").and_then(|v| v.as_array()) {
                            for item in current {
                                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                                    if let Some(home) = item.get("home_project").and_then(|h| h.as_str()) {
                                        println!("{} ({})", name, home);
                                    } else {
                                        println!("{}", name);
                                    }
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
            ProjectCommand::Pin => {
                client.post("/project/pin")?;
                Ok(())
            }
            ProjectCommand::Debug => {
                let response = client.get("/project/debug")?;
                println!("{}", response);
                Ok(())
            }
            ProjectCommand::Status { name, output } => {
                let path = match name {
                    Some(n) => format!("/project/status/{}", n),
                    None => "/project/status".to_string(),
                };
                let query = if output == "json" { "?format=json" } else { "" };
                let response = client.get(&format!("{}{}", path, query))?;
                print!("{}", response);
                Ok(())
            }
        },

        Command::File { path, land_in } => {
            let query = build_query(&land_in, &None);
            let url_path = format!("/file/{}{}", path, query);
            client.get(&url_path)?;
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

        Command::Jira { command } => match command {
            JiraCommand::Sprint { command } => match command {
                None => jira::print_sprint_issues(),
                Some(SprintCommand::Create { overrides }) => {
                    create_sprint_tasks(&client, overrides)
                }
            },
        },

        Command::Completion { shell, projects } => {
            if projects {
                let response = client.get("/project/list")?;
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                    if let Some(current) = json.get("current").and_then(|v| v.as_array()) {
                        for item in current {
                            if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                                println!("{}", name);
                            }
                        }
                    }
                }
            } else if let Some(shell) = shell {
                generate(shell, &mut Cli::command(), "wormhole", &mut io::stdout());
            }
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

fn create_sprint_tasks(client: &Client, overrides: Vec<String>) -> Result<(), String> {
    use std::collections::HashMap;
    use std::env;

    let default_home = env::var("WORMHOLE_DEFAULT_HOME_PROJECT").ok();

    // Parse override pairs
    let mut home_overrides: HashMap<String, String> = HashMap::new();
    let mut iter = overrides.iter();
    while let Some(ticket) = iter.next() {
        let home = iter
            .next()
            .ok_or_else(|| format!("Missing home project for ticket {}", ticket))?;
        home_overrides.insert(ticket.clone(), home.clone());
    }

    // Get sprint issues
    let issues = jira::get_sprint_issues()?;

    // Get existing tasks
    let response = client.get("/project/list")?;
    let existing: std::collections::HashSet<String> =
        serde_json::from_str::<serde_json::Value>(&response)
            .ok()
            .and_then(|v| v.get("current")?.as_array().cloned())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.get("name")?.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

    let mut created = 0;
    let mut skipped = 0;

    for issue in &issues {
        if existing.contains(&issue.key) {
            skipped += 1;
            continue;
        }

        let home = home_overrides
            .get(&issue.key)
            .or(default_home.as_ref())
            .ok_or_else(|| {
                format!(
                    "No home project for {} (set WORMHOLE_DEFAULT_HOME_PROJECT or provide override)",
                    issue.key
                )
            })?;

        let path = format!("/project/switch/{}?home-project={}", issue.key, home);
        client.get(&path)?;
        println!("Created task {} (home: {})", issue.key, home);
        created += 1;
    }

    println!(
        "\nCreated {} tasks, skipped {} (already exist)",
        created, skipped
    );
    Ok(())
}
