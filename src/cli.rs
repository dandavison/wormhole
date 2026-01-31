use clap::builder::ValueHint;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use serde::Serialize;
use std::io;

use crate::config;
use crate::jira;

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
pub enum TaskCommand {
    /// Create a task from a JIRA URL
    Create {
        /// JIRA URL (e.g., https://temporalio.atlassian.net/browse/ACT-622)
        url: String,
        /// Home project for the worktree
        #[arg(short = 'p', long)]
        home_project: Option<String>,
    },
    /// Create tasks from current sprint issues
    CreateFromSprint,
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

    /// Task operations (create from JIRA URL or sprint)
    Task {
        #[command(subcommand)]
        command: TaskCommand,
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

// Shared helper for project name tab completion
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Config, Context, Editor, Helper};

struct ProjectCompleter {
    projects: Vec<String>,
}
impl Helper for ProjectCompleter {}
impl Validator for ProjectCompleter {}
impl Hinter for ProjectCompleter {
    type Hint = String;
}
impl Highlighter for ProjectCompleter {}
impl Completer for ProjectCompleter {
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

fn create_project_editor(
    available_projects: Vec<String>,
) -> Result<Editor<ProjectCompleter, rustyline::history::DefaultHistory>, String> {
    let config = Config::builder()
        .auto_add_history(false)
        .completion_type(rustyline::CompletionType::List)
        .build();
    let helper = ProjectCompleter {
        projects: available_projects,
    };
    let mut rl: Editor<ProjectCompleter, rustyline::history::DefaultHistory> =
        Editor::with_config(config).map_err(|e| format!("Failed to init editor: {}", e))?;
    rl.set_helper(Some(helper));
    Ok(rl)
}

fn get_available_projects(client: &Client) -> Result<Vec<String>, String> {
    let response = client.get("/project/list")?;
    let parsed: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    Ok(parsed
        .get("available")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

fn create_task(client: &Client, home: &str, branch: &str, jira_key: &str) -> Result<(), String> {
    let url = format!("/project/create/{}?home-project={}", branch, home);
    client.get(&url)?;

    let store_key = format!("{}:{}", home, branch);
    let kv_url = format!("/kv/{}/jira_key", store_key);
    let _ = client.put(&kv_url, jira_key);

    Ok(())
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
                            println!("{}", render_project_item(item));
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
                let response = client.get(&path)?;
                if output == "json" {
                    println!("{}", response);
                } else {
                    let status: crate::status::TaskStatus =
                        serde_json::from_str(&response).map_err(|e| e.to_string())?;
                    println!("{}", render_task_status(&status));
                }
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
            },
        },

        Command::Task { command } => match command {
            TaskCommand::Create { url, home_project } => {
                task_create_from_url(&client, &url, home_project)
            }
            TaskCommand::CreateFromSprint => task_create_from_sprint(&client),
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

fn task_create_from_sprint(client: &Client) -> Result<(), String> {
    use std::collections::HashMap;
    use std::path::PathBuf;

    let default_home = std::env::var("WORMHOLE_DEFAULT_HOME_PROJECT").ok();

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
            let jira_key = v.get("kv")?.get("jira_key")?.as_str()?;
            let path = v.get("path")?.as_str()?;
            let branch = v.get("branch")?.as_str()?;
            Some((
                jira_key.to_string(),
                (PathBuf::from(path), branch.to_string()),
            ))
        })
        .collect();

    let available_projects = get_available_projects(client)?;
    let issues = jira::get_sprint_issues()?;

    let mut rl = create_project_editor(available_projects)?;
    let mut last_home = default_home;
    let mut created_count = 0;

    for issue in &issues {
        let existing = existing_tasks.get(&issue.key);
        let has_pr = existing
            .map(|(path, _)| crate::github::get_pr_status(path).is_some())
            .unwrap_or(false);

        if let Some(reason) = should_skip_issue(&issue.status, has_pr) {
            println!("{} {} [{}]", issue.key, issue.summary, reason);
            continue;
        }

        println!("\n{} {}", issue.key, issue.summary);

        let default_h = last_home.clone().unwrap_or_default();
        let home = match rl.readline_with_initial("home: ", (&default_h, "")) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    if default_h.is_empty() {
                        eprintln!("Skipping (no home)");
                        continue;
                    }
                    default_h
                } else {
                    trimmed.to_string()
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("Skipping");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => return Err(format!("Input error: {}", e)),
        };
        last_home = Some(home.clone());

        let default_branch = existing
            .map(|(_, branch)| branch.clone())
            .unwrap_or_else(|| to_kebab_case(&issue.summary));

        let branch = match rl.readline_with_initial("branch: ", (&default_branch, "")) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    default_branch
                } else {
                    trimmed.to_string()
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("Skipping");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => return Err(format!("Input error: {}", e)),
        };

        create_task(client, &home, &branch, &issue.key)?;
        println!("Created {}:{}", home, branch);
        created_count += 1;
    }

    if created_count > 0 {
        let _ = client.post("/project/refresh");
    }

    Ok(())
}

fn task_create_from_url(
    client: &Client,
    url: &str,
    home_project: Option<String>,
) -> Result<(), String> {
    let jira_key = crate::describe::parse_jira_url(url)
        .ok_or_else(|| format!("Could not parse JIRA key from URL: {}", url))?;

    let issue =
        jira::get_issue(&jira_key)?.ok_or_else(|| format!("JIRA issue not found: {}", jira_key))?;

    println!("{} {}", jira_key, issue.summary);

    let available_projects = get_available_projects(client)?;
    let mut rl = create_project_editor(available_projects)?;

    let home = if let Some(h) = home_project {
        h
    } else {
        let default_home = std::env::var("WORMHOLE_DEFAULT_HOME_PROJECT").ok();
        let default_h = default_home.unwrap_or_default();
        match rl.readline_with_initial("home: ", (&default_h, "")) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    if default_h.is_empty() {
                        return Err("Home project is required".to_string());
                    }
                    default_h
                } else {
                    trimmed.to_string()
                }
            }
            Err(_) => return Err("Aborted".to_string()),
        }
    };

    let default_branch = to_kebab_case(&issue.summary);
    let branch = match rl.readline_with_initial("branch: ", (&default_branch, "")) {
        Ok(line) => {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                default_branch
            } else {
                trimmed.to_string()
            }
        }
        Err(_) => return Err("Aborted".to_string()),
    };

    println!("Creating {}:{}", home, branch);

    create_task(client, &home, &branch, &jira_key)?;

    // Refresh cache so dashboard shows JIRA link immediately
    let _ = client.post("/project/refresh");

    println!("Created task {}:{} for {}", home, branch, jira_key);
    Ok(())
}

fn sprint_list(client: &Client, output: &str) -> Result<(), String> {
    use std::collections::HashSet;

    // Fetch sprint issues (client-side I/O)
    let sprint_keys: HashSet<String> = jira::get_sprint_issues()
        .map(|issues| issues.into_iter().map(|i| i.key).collect())
        .unwrap_or_default();

    // Get project list from server (in-memory, includes cached JIRA/PR)
    let response = client.get("/project/list")?;
    let json: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;

    // Filter to tasks with jira_key in sprint
    let sprint_tasks: Vec<&serde_json::Value> = json
        .get("current")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|item| {
                    item.get("kv")
                        .and_then(|kv| kv.get("jira_key"))
                        .and_then(|k| k.as_str())
                        .is_some_and(|k| sprint_keys.contains(k))
                })
                .collect()
        })
        .unwrap_or_default();

    if output == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&sprint_tasks).map_err(|e| e.to_string())?
        );
    } else {
        for item in sprint_tasks {
            println!("{}", render_project_item(item));
        }
    }
    Ok(())
}

/// Render a project item from /project/list response
fn render_project_item(item: &serde_json::Value) -> String {
    let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let store_key = if let Some(branch) = item.get("branch").and_then(|b| b.as_str()) {
        format!("{}:{}", name, branch)
    } else {
        name.to_string()
    };
    let task_url = format!(
        "http://127.0.0.1:{}/project/switch/{}",
        config::wormhole_port(),
        store_key
    );
    let task_display = crate::format_osc8_hyperlink(&task_url, &store_key);

    let jira_instance = std::env::var("JIRA_INSTANCE").ok();
    let (jira_key, status) = item
        .get("jira")
        .map(|j| {
            (
                j.get("key").and_then(|k| k.as_str()).unwrap_or(""),
                j.get("status").and_then(|s| s.as_str()).unwrap_or(""),
            )
        })
        .unwrap_or(("", ""));

    let pr_display = item
        .get("pr")
        .map(|p| {
            let url = p.get("url").and_then(|u| u.as_str()).unwrap_or("");
            let number = p.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
            let is_draft = p.get("isDraft").and_then(|d| d.as_bool()).unwrap_or(false);
            let display = if is_draft {
                format!("#{} (draft)", number)
            } else {
                format!("#{}", number)
            };
            format!("  {}", crate::format_osc8_hyperlink(url, &display))
        })
        .unwrap_or_default();

    // No JIRA info - just show the task identifier
    if jira_key.is_empty() {
        return task_display;
    }

    let jira_display = if let Some(ref instance) = jira_instance {
        let url = format!("https://{}.atlassian.net/browse/{}", instance, jira_key);
        crate::format_osc8_hyperlink(&url, jira_key)
    } else {
        jira_key.to_string()
    };

    let emoji = jira::status_emoji(status);
    let pad = 40_usize.saturating_sub(store_key.len());

    format!(
        "{} {}{} {}{}",
        emoji,
        task_display,
        " ".repeat(pad),
        jira_display,
        pr_display
    )
}

fn sprint_show(output: &str) -> Result<(), String> {
    use crate::status::TaskStatus;
    use std::thread;

    let issues = jira::get_sprint_issues()?;

    // Fetch status for each issue concurrently
    let statuses: Vec<_> = issues
        .iter()
        .map(|issue| {
            let key = issue.key.clone();
            let client_url = format!("http://127.0.0.1:{}", crate::config::wormhole_port());
            thread::spawn(move || {
                ureq::get(&format!("{}/project/show/{}", client_url, key))
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

    #[derive(serde::Serialize)]
    #[serde(tag = "type")]
    enum SprintShowItem {
        #[serde(rename = "task")]
        Task(TaskStatus),
        #[serde(rename = "issue")]
        Issue(crate::jira::IssueStatus),
    }

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
            match item {
                SprintShowItem::Task(task) => println!("{}\n\n", render_task_status(task)),
                SprintShowItem::Issue(issue) => {
                    println!("{}\n  (no wormhole task)\n\n", render_issue_status(issue))
                }
            }
        }
    }
    Ok(())
}

fn render_task_status(status: &crate::status::TaskStatus) -> String {
    let jira_instance = std::env::var("JIRA_INSTANCE").ok();

    let name_linked = if let Some(ref instance) = jira_instance {
        let url = format!("https://{}.atlassian.net/browse/{}", instance, status.name);
        crate::format_osc8_hyperlink(&url, &status.name)
    } else {
        status.name.clone()
    };

    let title = if let Some(ref jira) = status.jira {
        format!("{}: {}", name_linked, jira.summary)
    } else {
        name_linked.clone()
    };
    let title_len = if let Some(ref jira) = status.jira {
        status.name.len() + 2 + jira.summary.len()
    } else {
        status.name.len()
    };

    let mut lines = vec![title, "─".repeat(title_len)];

    if let Some(ref branch) = status.branch {
        lines.push(format!("Branch:    {}", branch));
    }

    if let Some(ref jira) = status.jira {
        lines.push(format!(
            "JIRA:      {} {}",
            crate::jira::status_emoji(&jira.status),
            jira.status
        ));
    } else if status.branch.is_some() {
        lines.push("JIRA:      ✗".to_string());
    }

    if let Some(ref pr) = status.pr {
        let pr_linked = crate::format_osc8_hyperlink(&pr.url, &pr.display());
        let comments = pr
            .comments_display()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();
        lines.push(format!("PR:        {}{}", pr_linked, comments));
    } else {
        lines.push("PR:        ✗".to_string());
    }

    if let Some(ref url) = status.plan_url {
        let plan_linked = crate::format_osc8_hyperlink(url, "✓ plan.md");
        lines.push(format!("Plan:      {}", plan_linked));
    } else {
        lines.push("Plan:      ✗".to_string());
    }

    if let Some(ref repos) = status.aux_repos {
        lines.push(format!("Aux repos: {}", repos));
    } else {
        lines.push("Aux repos: ✗".to_string());
    }

    lines.join("\n")
}

fn render_issue_status(issue: &crate::jira::IssueStatus) -> String {
    format!(
        "{} {}: {}",
        crate::jira::status_emoji(&issue.status),
        issue.key,
        issue.summary
    )
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

    #[test]
    fn test_render_project_item_bare_project() {
        // Project without branch or JIRA - just shows name
        let item = serde_json::json!({
            "name": "wormhole",
            "path": "/Users/dan/src/wormhole"
        });
        let rendered = render_project_item(&item);
        // Should contain the project name (inside a hyperlink)
        assert!(rendered.contains("wormhole"), "Should contain project name");
        // Should not contain emoji (no JIRA)
        assert!(
            !rendered.contains("⚫"),
            "Should not have emoji without JIRA"
        );
    }

    #[test]
    fn test_render_project_item_task_without_jira() {
        // Task (has branch) but no JIRA - shows repo:branch
        let item = serde_json::json!({
            "name": "cli",
            "branch": "feature-branch",
            "path": "/Users/dan/src/cli/feature-branch"
        });
        let rendered = render_project_item(&item);
        assert!(
            rendered.contains("cli:feature-branch"),
            "Should show repo:branch format"
        );
        assert!(
            !rendered.contains("⚫"),
            "Should not have emoji without JIRA"
        );
    }

    #[test]
    fn test_render_project_item_task_with_jira() {
        // Task with JIRA info - shows emoji, task, JIRA key (no summary)
        let item = serde_json::json!({
            "name": "cli",
            "branch": "standalone-activity",
            "path": "/Users/dan/src/cli/standalone-activity",
            "jira": {
                "key": "ACT-107",
                "status": "In Progress",
                "summary": "Standalone activity CLI integration"
            }
        });
        let rendered = render_project_item(&item);
        assert!(
            rendered.starts_with("🔵"),
            "Should start with In Progress emoji"
        );
        assert!(
            rendered.contains("cli:standalone-activity"),
            "Should contain task identifier"
        );
        assert!(rendered.contains("ACT-107"), "Should contain JIRA key");
        assert!(
            !rendered.contains("Standalone activity CLI integration"),
            "Should not contain summary"
        );
    }

    #[test]
    fn test_render_project_item_task_with_pr() {
        // Task with JIRA and PR
        let item = serde_json::json!({
            "name": "cli",
            "branch": "feature",
            "jira": {
                "key": "ACT-100",
                "status": "In Review",
                "summary": "Feature work"
            },
            "pr": {
                "number": 123,
                "url": "https://github.com/org/cli/pull/123",
                "isDraft": false
            }
        });
        let rendered = render_project_item(&item);
        assert!(rendered.contains("#123"), "Should contain PR number");
    }

    #[test]
    fn test_render_project_item_draft_pr() {
        let item = serde_json::json!({
            "name": "cli",
            "branch": "feature",
            "jira": {
                "key": "ACT-100",
                "status": "In Review",
                "summary": "Feature work"
            },
            "pr": {
                "number": 456,
                "url": "https://github.com/org/cli/pull/456",
                "isDraft": true
            }
        });
        let rendered = render_project_item(&item);
        assert!(rendered.contains("#456 (draft)"), "Should show draft PR");
    }
}
