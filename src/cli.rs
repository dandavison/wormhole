use clap::builder::ValueHint;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use serde::Serialize;
use std::io;

use crate::config;
use crate::jira;

#[derive(Serialize)]
struct SprintCreateResult {
    created: Vec<String>,
    skipped: Vec<String>,
    no_home: Vec<String>,
}

impl SprintCreateResult {
    fn render_terminal(&self) -> String {
        let mut lines = Vec::new();
        for task in &self.created {
            lines.push(format!("Created task {}", task));
        }
        for key in &self.no_home {
            lines.push(format!("Skipping {} (no home project)", key));
        }
        lines.push(format!(
            "\nCreated {} tasks, skipped {} (already exist)",
            self.created.len(),
            self.skipped.len()
        ));
        lines.join("\n")
    }
}

#[derive(Serialize, serde::Deserialize)]
struct ProjectDebug {
    index: usize,
    name: String,
    path: String,
    aliases: Vec<String>,
    home_project: Option<String>,
}

impl ProjectDebug {
    fn render_terminal(&self) -> String {
        let aliases = if self.aliases.is_empty() {
            "none".to_string()
        } else {
            self.aliases.join(", ")
        };
        format!(
            "[{}] name: {}, path: {}, aliases: [{}]",
            self.index, self.name, self.path, aliases
        )
    }
}

#[derive(Serialize)]
struct KvValue {
    project: String,
    key: String,
    value: Option<String>,
}

impl KvValue {
    fn render_terminal(&self) -> String {
        self.value.clone().unwrap_or_default()
    }
}

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
    /// List sprint issues
    List {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
    },
    /// Show detailed status for each sprint issue
    Show {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
    },
    /// Create tasks for sprint issues
    Create {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
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
        /// List available projects (from WORMHOLE_PATH) instead of current
        #[arg(short, long)]
        available: bool,
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
    Debug {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
    },
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
        #[arg(value_enum, required_unless_present_any = ["projects", "available"])]
        shell: Option<Shell>,
        /// Output current project names (for dynamic completion)
        #[arg(long)]
        projects: bool,
        /// Output available project names (for dynamic completion)
        #[arg(long)]
        available: bool,
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
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
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
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
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
            ProjectCommand::List { output, available } => {
                let response = client.get("/project/list")?;
                if output == "json" {
                    println!("{}", response);
                } else if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                    if available {
                        if let Some(avail) = json.get("available").and_then(|v| v.as_array()) {
                            for item in avail {
                                if let Some(name) = item.as_str() {
                                    println!("{}", name);
                                }
                            }
                        }
                    } else if let Some(current) = json.get("current").and_then(|v| v.as_array()) {
                        for item in current {
                            if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                                if let Some(home) =
                                    item.get("home_project").and_then(|h| h.as_str())
                                {
                                    println!("{} ({})", name, home);
                                } else {
                                    println!("{}", name);
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
            ProjectCommand::Debug { output } => {
                let response = client.get("/project/debug")?;
                if output == "json" {
                    println!("{}", response);
                } else {
                    let projects: Vec<ProjectDebug> = serde_json::from_str(&response)
                        .map_err(|e| format!("Failed to parse debug response: {}", e))?;
                    for p in &projects {
                        println!("{}", p.render_terminal());
                    }
                }
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
            KvCommand::Get {
                project,
                key,
                output,
            } => {
                let response = client.get(&format!("/kv/{}/{}", project, key));
                let kv = KvValue {
                    project: project.clone(),
                    key: key.clone(),
                    value: response.ok(),
                };
                if output == "json" {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&kv).map_err(|e| e.to_string())?
                    );
                } else {
                    println!("{}", kv.render_terminal());
                }
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
            KvCommand::List { project, output } => {
                let path = match &project {
                    Some(p) => format!("/kv/{}", p),
                    None => "/kv".to_string(),
                };
                let response = client.get(&path)?;
                if output == "json" {
                    println!("{}", response);
                } else {
                    // Parse JSON and render text
                    if let Ok(kv) =
                        serde_json::from_str::<std::collections::HashMap<String, String>>(&response)
                    {
                        for (k, v) in &kv {
                            println!("{}: {}", k, v);
                        }
                    } else {
                        println!("{}", response);
                    }
                }
                Ok(())
            }
        },

        Command::Jira { command } => match command {
            JiraCommand::Sprint { command } => match command {
                None => sprint_list("text"),
                Some(SprintCommand::List { output }) => sprint_list(&output),
                Some(SprintCommand::Show { output }) => sprint_show(&output),
                Some(SprintCommand::Create { output, overrides }) => {
                    sprint_create(&client, overrides, &output)
                }
            },
        },

        Command::Completion {
            shell,
            projects,
            available,
        } => {
            if projects || available {
                let response = client.get("/project/list")?;
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                    let key = if available { "available" } else { "current" };
                    if let Some(arr) = json.get(key).and_then(|v| v.as_array()) {
                        for item in arr {
                            let name = if available {
                                item.as_str()
                            } else {
                                item.get("name").and_then(|n| n.as_str())
                            };
                            if let Some(name) = name {
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

fn sprint_create(client: &Client, overrides: Vec<String>, output: &str) -> Result<(), String> {
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

    let mut result = SprintCreateResult {
        created: Vec::new(),
        skipped: Vec::new(),
        no_home: Vec::new(),
    };

    for issue in &issues {
        if existing.contains(&issue.key) {
            result.skipped.push(issue.key.clone());
            continue;
        }

        let home = match home_overrides.get(&issue.key).or(default_home.as_ref()) {
            Some(h) => h,
            None => {
                result.no_home.push(issue.key.clone());
                continue;
            }
        };

        let path = format!("/project/create/{}?home-project={}", issue.key, home);
        client.get(&path)?;
        result.created.push(format!("{} ({})", issue.key, home));
    }

    if output == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
        );
    } else {
        println!("{}", result.render_terminal());
    }
    Ok(())
}

fn sprint_list(output: &str) -> Result<(), String> {
    let issues = jira::get_sprint_issues()?;
    if output == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&issues).map_err(|e| e.to_string())?
        );
    } else {
        for issue in &issues {
            println!("{}", issue.render_terminal());
        }
    }
    Ok(())
}

fn sprint_show(output: &str) -> Result<(), String> {
    use crate::status::{SprintShowItem, TaskStatus};
    use std::thread;

    let issues = jira::get_sprint_issues()?;

    // Fetch status for each issue concurrently (as JSON)
    let statuses: Vec<_> = issues
        .iter()
        .map(|issue| {
            let key = issue.key.clone();
            let client_url = format!("http://127.0.0.1:{}", crate::config::wormhole_port());
            thread::spawn(move || {
                ureq::get(&format!(
                    "{}/project/status/{}?format=json",
                    client_url, key
                ))
                .call()
                .ok()
                .and_then(|r| r.into_string().ok())
                .and_then(|s| serde_json::from_str::<TaskStatus>(&s).ok())
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().ok().flatten())
        .collect();

    // Build SprintShowItem list
    let items: Vec<SprintShowItem> = issues
        .into_iter()
        .zip(statuses.into_iter())
        .map(|(issue, status)| match status {
            Some(task) => SprintShowItem::Task(task),
            None => SprintShowItem::Issue(issue),
        })
        .collect();

    if output == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&items).map_err(|e| e.to_string())?
        );
    } else {
        for item in &items {
            println!("{}\n", item.render_terminal());
        }
    }
    Ok(())
}

