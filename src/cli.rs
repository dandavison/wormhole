use clap::builder::ValueHint;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use serde::Serialize;
use std::io;

use crate::config;
use crate::jira;

#[derive(Serialize)]
struct CreatedTask {
    home: String,
    key: String,
    summary: String,
}

impl CreatedTask {
    fn render_terminal(&self) -> String {
        format!("{:<18} {:<10} {}", self.home, self.key, self.summary)
    }
}

#[derive(Serialize)]
struct SprintCreateResult {
    created: Vec<CreatedTask>,
    skipped: Vec<String>,
    no_home: Vec<String>,
}

impl SprintCreateResult {
    fn render_terminal(&self) -> String {
        let mut lines: Vec<String> = self.created.iter().map(|t| t.render_terminal()).collect();
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
    home_project: Option<String>,
}

impl ProjectDebug {
    fn render_terminal(&self) -> String {
        format!("[{}] name: {}, path: {}", self.index, self.name, self.path)
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
        /// Branch name for new task (prompted if not provided)
        #[arg(long)]
        branch: Option<String>,
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
    /// Show project/task info (JIRA, PR, etc.)
    Show {
        /// Project name (defaults to current project)
        name: Option<String>,
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
    },
}

#[derive(Subcommand)]
pub enum ServerCommand {
    /// Run server in foreground (used internally by daemon)
    #[command(hide = true)]
    StartForeground,
    /// Start the server daemon (background)
    Start,
    /// Stop the server daemon
    Stop,
    /// Attach to the running server daemon
    Attach,
}

#[derive(Subcommand)]
pub enum Command {
    /// Server daemon operations
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },

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
    Kill,

    /// Diagnostic commands
    Doctor {
        #[command(subcommand)]
        command: DoctorCommand,
    },

    /// Refresh in-memory data from external sources
    Refresh,
}

#[derive(Subcommand)]
pub enum DoctorCommand {
    /// Report on persisted wormhole data (worktrees, KV files on disk)
    PersistedData {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
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
            .map_err(map_ureq_error)?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    fn post(&self, path: &str) -> Result<String, String> {
        ureq::post(&format!("{}{}", self.base_url, path))
            .call()
            .map_err(map_ureq_error)?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    fn put(&self, path: &str, body: &str) -> Result<String, String> {
        ureq::put(&format!("{}{}", self.base_url, path))
            .send_string(body)
            .map_err(map_ureq_error)?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    fn delete(&self, path: &str) -> Result<String, String> {
        ureq::delete(&format!("{}{}", self.base_url, path))
            .call()
            .map_err(map_ureq_error)?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }
}

fn map_ureq_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(_code, response) => response
            .into_string()
            .unwrap_or_else(|_| "Unknown error".to_string()),
        e => format!("Request failed: {}", e),
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
    branch: &Option<String>,
) -> String {
    let mut params = vec!["sync=true".to_string()];
    if let Some(app) = land_in {
        params.push(format!("land-in={}", app));
    }
    if let Some(n) = name {
        params.push(format!("name={}", n));
    }
    if let Some(h) = home_project {
        params.push(format!("home-project={}", h));
    }
    if let Some(b) = branch {
        params.push(format!("branch={}", b));
    }
    format!("?{}", params.join("&"))
}

fn prompt_for_branch_if_needed(
    client: &Client,
    task_id: &str,
    home_project: &Option<String>,
    branch: Option<String>,
) -> Result<Option<String>, String> {
    if branch.is_some() {
        return Ok(branch);
    }
    if home_project.is_none() {
        return Ok(None);
    }
    // Check if task already exists via HTTP
    let response = client.get("/project/list")?;
    let task_exists = serde_json::from_str::<serde_json::Value>(&response)
        .ok()
        .and_then(|v| v.get("current")?.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .any(|v| v.get("name").and_then(|n| n.as_str()) == Some(task_id))
        })
        .unwrap_or(false);

    if task_exists {
        return Ok(None);
    }
    eprint!("Branch name for {}: ", task_id);
    io::Write::flush(&mut io::stderr()).ok();
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read branch name: {}", e))?;
    let branch = input.trim();
    if branch.is_empty() {
        return Err("Branch name required for new task".to_string());
    }
    Ok(Some(branch.to_string()))
}

pub fn run(command: Command) -> Result<(), String> {
    let client = Client::new();

    match command {
        Command::Server { command } => match command {
            ServerCommand::StartForeground => {
                unreachable!("StartForeground command should be handled in main")
            }
            ServerCommand::Start => {
                let tmux_env = std::env::var("TMUX")
                    .map_err(|_| "TMUX env var not set - run from within tmux")?;
                let d = wormhole::daemon::daemon();
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                d.start(
                    exe.to_str().ok_or("invalid exe path")?,
                    None,
                    None,
                    &[("WORMHOLE_TMUX", &tmux_env)],
                )?;
                println!("wormhole started");
                Ok(())
            }
            ServerCommand::Stop => {
                wormhole::daemon::daemon().stop();
                println!("wormhole stopped");
                Ok(())
            }
            ServerCommand::Attach => {
                let d = wormhole::daemon::daemon();
                if d.is_running() {
                    d.attach()
                } else {
                    Err("wormhole not running".to_string())
                }
            }
        },

        Command::Project { command } => match command {
            ProjectCommand::Switch {
                name_or_path,
                name,
                land_in,
                home_project,
                branch,
            } => {
                let branch =
                    prompt_for_branch_if_needed(&client, &name_or_path, &home_project, branch)?;
                let query = build_switch_query(&land_in, &name, &home_project, &branch);
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
                            let name = match item.get("name").and_then(|n| n.as_str()) {
                                Some(n) => n,
                                None => continue,
                            };
                            let store_key = if let Some(branch) =
                                item.get("branch").and_then(|b| b.as_str())
                            {
                                format!("{}:{}", name, branch)
                            } else {
                                name.to_string()
                            };
                            let url = format!(
                                "http://127.0.0.1:{}/project/switch/{}",
                                config::wormhole_port(),
                                store_key
                            );
                            println!("{}", crate::format_osc8_hyperlink(&url, &store_key));
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
            ProjectCommand::Show { name, output } => {
                let path = match name {
                    Some(n) => format!("/project/show/{}", n),
                    None => {
                        let cwd = std::env::current_dir()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        format!("/project/show/{}", cwd)
                    }
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
                None => sprint_list(&client, "text"),
                Some(SprintCommand::List { output }) => sprint_list(&client, &output),
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

        Command::Kill => {
            let _ = std::fs::remove_file("/tmp/wormhole.env");
            std::process::Command::new("tmux")
                .args(["kill-session"])
                .status()
                .map_err(|e| format!("Failed to kill tmux session: {}", e))?;
            Ok(())
        }

        Command::Doctor { command } => match command {
            DoctorCommand::PersistedData { output } => doctor_persisted_data(&output),
        },

        Command::Refresh => {
            client.post("/project/refresh")?;
            println!("Refreshed");
            Ok(())
        }
    }
}

#[derive(Serialize)]
struct PersistedDataReport {
    projects: Vec<ProjectPersistedData>,
}

#[derive(Serialize)]
struct ProjectPersistedData {
    name: String,
    path: String,
    worktrees: Vec<WorktreeInfo>,
    kv: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

#[derive(Serialize)]
struct WorktreeInfo {
    dir: String,
    branch: Option<String>,
}

impl PersistedDataReport {
    fn render_terminal(&self) -> String {
        if self.projects.is_empty() {
            return "No persisted wormhole data found.".to_string();
        }

        let mut lines = Vec::new();
        for project in &self.projects {
            lines.push(format!("{}:", project.name));
            lines.push(format!("  path: {}", project.path));

            if !project.worktrees.is_empty() {
                lines.push("  worktrees:".to_string());
                for wt in &project.worktrees {
                    let branch = wt.branch.as_deref().unwrap_or("(detached)");
                    lines.push(format!("    {} -> {}", wt.dir, branch));
                }
            }

            if !project.kv.is_empty() {
                lines.push("  kv:".to_string());
                for (file, pairs) in &project.kv {
                    lines.push(format!("    {}:", file));
                    for (k, v) in pairs {
                        lines.push(format!("      {}: {}", k, v));
                    }
                }
            }
            lines.push(String::new());
        }
        lines.join("\n")
    }
}

fn doctor_persisted_data(output: &str) -> Result<(), String> {
    use rayon::prelude::*;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    // Discover all available projects
    let available = config::available_projects();
    let repo_paths: Vec<(String, PathBuf)> = available.into_iter().collect();

    // Query each repo in parallel - only include repos with wormhole data
    let projects: Vec<ProjectPersistedData> = repo_paths
        .par_iter()
        .filter_map(|(name, path)| {
            if !crate::git::is_git_repo(path) {
                return None;
            }

            // Get worktrees
            let worktrees = crate::git::list_worktrees(path);
            let worktree_base = crate::git::worktree_base_path(path);
            let wormhole_worktrees: Vec<WorktreeInfo> = worktrees
                .into_iter()
                .filter(|wt| wt.path.starts_with(&worktree_base))
                .map(|wt| WorktreeInfo {
                    dir: wt
                        .path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?")
                        .to_string(),
                    branch: wt.branch,
                })
                .collect();

            // Read KV files
            let kv_dir = crate::git::git_common_dir(path).join("wormhole/kv");
            let mut all_kv: HashMap<String, HashMap<String, String>> = HashMap::new();
            if let Ok(entries) = fs::read_dir(&kv_dir) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if file_path.extension().map(|e| e == "json").unwrap_or(false) {
                        if let Ok(contents) = fs::read_to_string(&file_path) {
                            if let Ok(kv) =
                                serde_json::from_str::<HashMap<String, String>>(&contents)
                            {
                                let stem = file_path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                all_kv.insert(stem, kv);
                            }
                        }
                    }
                }
            }

            // Only include if there's wormhole data
            if wormhole_worktrees.is_empty() && all_kv.is_empty() {
                return None;
            }

            Some(ProjectPersistedData {
                name: name.clone(),
                path: path.display().to_string(),
                worktrees: wormhole_worktrees,
                kv: all_kv,
            })
        })
        .collect();

    let report = PersistedDataReport { projects };

    if output == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
    } else {
        println!("{}", report.render_terminal());
    }

    Ok(())
}

fn sprint_create(client: &Client, overrides: Vec<String>, output: &str) -> Result<(), String> {
    use rustyline::completion::{Completer, Pair};
    use rustyline::highlight::Highlighter;
    use rustyline::hint::Hinter;
    use rustyline::validate::Validator;
    use rustyline::{Config, Context, Editor, Helper};
    use std::collections::HashMap;
    use std::env;
    use std::path::PathBuf;

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

    // Get existing tasks from daemon (includes path and branch)
    let response = client.get("/project/list")?;
    let parsed: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    let current = parsed
        .get("current")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let existing_tasks: HashMap<String, (PathBuf, String)> = current
        .iter()
        .filter_map(|v| {
            let name = v.get("name")?.as_str()?;
            let path = v.get("path")?.as_str()?;
            let branch = v.get("branch")?.as_str().unwrap_or(name);
            Some((name.to_string(), (PathBuf::from(path), branch.to_string())))
        })
        .collect();

    // Get available projects for completion
    let available_projects: Vec<String> = parsed
        .get("available")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Get sprint issues
    let issues = jira::get_sprint_issues()?;

    struct ProjectHelper {
        projects: Vec<String>,
    }
    impl Helper for ProjectHelper {}
    impl Validator for ProjectHelper {}
    impl Hinter for ProjectHelper {
        type Hint = String;
    }
    impl Highlighter for ProjectHelper {}
    impl Completer for ProjectHelper {
        type Candidate = Pair;
        fn complete(
            &self,
            line: &str,
            pos: usize,
            _ctx: &Context,
        ) -> rustyline::Result<(usize, Vec<Pair>)> {
            let prefix = &line[..pos];
            let candidates: Vec<Pair> = self
                .projects
                .iter()
                .filter(|p| p.starts_with(prefix))
                .map(|p| Pair {
                    display: p.clone(),
                    replacement: p.clone(),
                })
                .collect();
            Ok((0, candidates))
        }
    }

    let config = Config::builder()
        .auto_add_history(false)
        .completion_type(rustyline::CompletionType::List)
        .build();
    let helper = ProjectHelper {
        projects: available_projects,
    };
    let mut rl =
        Editor::with_config(config).map_err(|e| format!("Failed to init editor: {}", e))?;
    rl.set_helper(Some(helper));

    let mut result = SprintCreateResult {
        created: Vec::new(),
        skipped: Vec::new(),
        no_home: Vec::new(),
    };

    // Filter issues that will be skipped
    let actionable: Vec<_> = issues
        .iter()
        .filter(|issue| {
            let existing = existing_tasks.get(&issue.key);
            let has_pr = existing
                .map(|(path, _)| crate::github::get_pr_status(path).is_some())
                .unwrap_or(false);
            if should_skip_issue(&issue.status, has_pr).is_some() {
                result.skipped.push(issue.key.clone());
                false
            } else {
                true
            }
        })
        .collect();

    if actionable.is_empty() {
        print_sprint_result(&result, output);
        return Ok(());
    }

    // Collect home repo and branch for each issue, with confirmation loop
    let to_create = loop {
        let mut entries: Vec<(String, String, String)> = Vec::new(); // (key, home, branch)
        let mut aborted = false;
        let mut last_home = default_home.clone();

        for issue in &actionable {
            println!("\n{} {}", issue.key, issue.summary);

            // Prompt for home repo
            let default_h = home_overrides
                .get(&issue.key)
                .cloned()
                .or(last_home.clone())
                .unwrap_or_default();
            let issue_home = match rl.readline_with_initial("  home: ", (&default_h, "")) {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        if default_h.is_empty() {
                            eprintln!("Home repo is required.");
                            aborted = true;
                            break;
                        }
                        default_h
                    } else {
                        trimmed.to_string()
                    }
                }
                Err(rustyline::error::ReadlineError::Interrupted)
                | Err(rustyline::error::ReadlineError::Eof) => {
                    aborted = true;
                    break;
                }
                Err(e) => return Err(format!("Input error: {}", e)),
            };
            last_home = Some(issue_home.clone());

            // Prompt for branch
            let existing = existing_tasks.get(&issue.key);
            let default_branch = existing
                .map(|(_, branch)| branch.clone())
                .unwrap_or_else(|| to_kebab_case(&issue.summary));

            let branch = match rl.readline_with_initial("  branch: ", (&default_branch, "")) {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        default_branch
                    } else {
                        trimmed.to_string()
                    }
                }
                Err(rustyline::error::ReadlineError::Interrupted)
                | Err(rustyline::error::ReadlineError::Eof) => {
                    aborted = true;
                    break;
                }
                Err(e) => return Err(format!("Input error: {}", e)),
            };

            entries.push((issue.key.clone(), issue_home, branch));
        }

        if aborted {
            eprintln!("Aborted.");
            return Ok(());
        }

        // Show confirmation
        println!("\nWill create:");
        for (key, h, branch) in &entries {
            println!("  {:18} {:18} {}", h, key, branch);
        }

        let confirm = rl.readline("\n[c]reate, [r]edo, [a]bort? ");
        match confirm {
            Ok(s) if s.trim().eq_ignore_ascii_case("c") => break entries,
            Ok(s) if s.trim().eq_ignore_ascii_case("a") => {
                eprintln!("Aborted.");
                return Ok(());
            }
            Ok(s) if s.trim().eq_ignore_ascii_case("r") => continue,
            Err(rustyline::error::ReadlineError::Interrupted)
            | Err(rustyline::error::ReadlineError::Eof) => {
                eprintln!("Aborted.");
                return Ok(());
            }
            _ => continue,
        }
    };

    for (key, home, branch) in to_create {
        let url = format!("/project/create/{}?home-project={}", branch, home);
        client.get(&url)?;
        // Store JIRA key in task's KV
        let store_key = format!("{}:{}", home, branch);
        let kv_url = format!("/kv/{}/jira_key", store_key);
        let _ = client.put(&kv_url, &key);
        result.created.push(CreatedTask {
            home,
            key,
            summary: branch,
        });
    }

    print_sprint_result(&result, output);
    Ok(())
}

fn print_sprint_result(result: &SprintCreateResult, output: &str) {
    if output == "json" {
        if let Ok(json) = serde_json::to_string_pretty(result) {
            println!("{}", json);
        }
    } else {
        println!("{}", result.render_terminal());
    }
}

fn sprint_list(client: &Client, output: &str) -> Result<(), String> {
    use crate::status::SprintShowItem;
    use std::env;

    let response = client.get("/api/sprint")?;
    let items: Vec<SprintShowItem> =
        serde_json::from_str(&response).map_err(|e| e.to_string())?;

    if output == "json" {
        println!("{}", response);
    } else {
        let jira_instance = env::var("JIRA_INSTANCE").ok();
        for item in &items {
            println!("{}", render_sprint_list_item(item, jira_instance.as_deref()));
        }
    }
    Ok(())
}

fn render_sprint_list_item(item: &crate::status::SprintShowItem, jira_instance: Option<&str>) -> String {
    use crate::status::SprintShowItem;

    match item {
        SprintShowItem::Task(task) => {
            let store_key = task
                .branch
                .as_ref()
                .map(|b| format!("{}:{}", task.name, b))
                .unwrap_or_else(|| task.name.clone());
            let task_url = format!(
                "http://127.0.0.1:{}/project/switch/{}",
                config::wormhole_port(),
                store_key
            );
            let task_display = crate::format_osc8_hyperlink(&task_url, &store_key);
            let (jira_key, status, summary) = task
                .jira
                .as_ref()
                .map(|j| (j.key.clone(), j.status.clone(), j.summary.clone()))
                .unwrap_or_else(|| (task.name.clone(), String::new(), String::new()));
            let jira_display = if let Some(instance) = jira_instance {
                let url = format!("https://{}.atlassian.net/browse/{}", instance, jira_key);
                crate::format_osc8_hyperlink(&url, &jira_key)
            } else {
                jira_key.clone()
            };
            let pr_display = task
                .pr
                .as_ref()
                .map(|p| format!("  {}", crate::format_osc8_hyperlink(&p.url, &p.display())))
                .unwrap_or_default();
            let emoji = status_to_emoji(&status);
            // Pad store_key length (not the hyperlink) for alignment
            let pad = 40_usize.saturating_sub(store_key.len());
            format!(
                "{}{} {} {}  {}{}",
                task_display,
                " ".repeat(pad),
                emoji,
                jira_display,
                summary,
                pr_display
            )
        }
        SprintShowItem::Issue(issue) => {
            let jira_display = if let Some(instance) = jira_instance {
                let url = format!("https://{}.atlassian.net/browse/{}", instance, issue.key);
                crate::format_osc8_hyperlink(&url, &issue.key)
            } else {
                issue.key.clone()
            };
            let emoji = status_to_emoji(&issue.status);
            format!("{:40} {} {}  {}", "", emoji, jira_display, issue.summary)
        }
    }
}

fn status_to_emoji(status: &str) -> &'static str {
    match status {
        "Done" => "✅",
        "In Progress" => "🟢",
        "In Review" => "🔵",
        _ => "⚫",
    }
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
                ureq::get(&format!("{}/project/show/{}?format=json", client_url, key))
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
        .zip(statuses)
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
            println!("{}\n\n", item.render_terminal());
        }
    }
    Ok(())
}

fn to_kebab_case(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() || c == '-' || c == '_' {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Determines if we should prompt for a branch name for this issue.
/// Returns None if we should prompt, or Some(reason) if we should skip.
fn should_skip_issue(status: &str, has_pr: bool) -> Option<&'static str> {
    let status_lower = status.to_lowercase();
    if status_lower == "done" || status_lower == "closed" || status_lower == "resolved" {
        return Some("done");
    }
    if has_pr {
        return Some("has_pr");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("Hello World"), "hello-world");
        assert_eq!(
            to_kebab_case("[Jan] Standalone activity CLI integration"),
            "jan-standalone-activity-cli-integration"
        );
        assert_eq!(
            to_kebab_case("Fix PollActivityTaskQueueResponse proto doc comments"),
            "fix-pollactivitytaskqueueresponse-proto-doc-comments"
        );
        assert_eq!(to_kebab_case("Multiple   spaces"), "multiple-spaces");
    }

    #[test]
    fn test_should_skip_done_issues() {
        assert_eq!(should_skip_issue("Done", false), Some("done"));
        assert_eq!(should_skip_issue("DONE", false), Some("done"));
        assert_eq!(should_skip_issue("Closed", false), Some("done"));
        assert_eq!(should_skip_issue("Resolved", false), Some("done"));
    }

    #[test]
    fn test_should_skip_issues_with_pr() {
        assert_eq!(should_skip_issue("In Progress", true), Some("has_pr"));
        assert_eq!(should_skip_issue("In Review", true), Some("has_pr"));
    }

    #[test]
    fn test_should_prompt_for_active_issues_without_pr() {
        assert_eq!(should_skip_issue("In Progress", false), None);
        assert_eq!(should_skip_issue("In Review", false), None);
        assert_eq!(should_skip_issue("To Do", false), None);
        assert_eq!(should_skip_issue("Open", false), None);
        assert_eq!(should_skip_issue("Backlog", false), None);
        assert_eq!(should_skip_issue("Selected for Development", false), None);
    }

    #[test]
    fn test_done_takes_priority_over_has_pr() {
        // If issue is done, skip reason should be "done" even if it has a PR
        assert_eq!(should_skip_issue("Done", true), Some("done"));
        assert_eq!(should_skip_issue("Closed", true), Some("done"));
    }
}
