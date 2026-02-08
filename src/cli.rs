use clap::builder::ValueHint;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::engine::{
    ArgValueCompleter, CompletionCandidate, PathCompleter, ValueCompleter,
};
use clap_complete::{generate, Shell};
use serde::Serialize;
use std::io;

use crate::config;
use crate::jira;
use crate::project::ProjectKey;
use crate::pst::TerminalHyperlink;

fn complete_projects(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let mut candidates = PathCompleter::any().complete(current);

    let url = format!("http://127.0.0.1:{}/project/list", config::wormhole_port());
    let response = match ureq::get(&url).call() {
        Ok(r) => match r.into_string() {
            Ok(s) => s,
            Err(_) => return candidates,
        },
        Err(_) => return candidates,
    };
    let json: serde_json::Value = match serde_json::from_str(&response) {
        Ok(v) => v,
        Err(_) => return candidates,
    };
    if let Some(current) = json.get("current").and_then(|v| v.as_array()) {
        for item in current {
            if let Some(key) = item.get("project_key").and_then(|k| k.as_str()) {
                candidates.push(CompletionCandidate::new(key));
            }
        }
    }
    if let Some(available) = json.get("available").and_then(|v| v.as_array()) {
        for item in available {
            if let Some(name) = item.as_str() {
                if !candidates
                    .iter()
                    .any(|c| c.get_value().to_str() == Some(name))
                {
                    candidates.push(CompletionCandidate::new(name));
                }
            }
        }
    }
    candidates
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
        let name_linked = ProjectKey::parse(&self.name).hyperlink();
        format!(
            "[{}] name: {}, path: {}",
            self.index, name_linked, self.path
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
pub enum TaskCommand {
    /// Create or update a task
    Upsert {
        /// Target: project key (repo:branch), JIRA URL, or JIRA key (ACT-123)
        #[arg(add = ArgValueCompleter::new(complete_projects))]
        target: String,
        /// Home project for the worktree (required for create)
        #[arg(short = 'p', long, add = ArgValueCompleter::new(complete_projects))]
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
    /// List projects (current and available)
    List {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
        /// List available projects (from WORMHOLE_PATH) instead of current
        #[arg(short, long)]
        available: bool,

        // TODO : is this used? What are the respective roles of this and complete_projects at the
        // top of this file?
        /// Output only project names (for shell completion)
        #[arg(long)]
        name_only: bool,
        /// List only projects with a tmux window
        #[arg(long)]
        active: bool,
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
        #[arg(add = ArgValueCompleter::new(complete_projects))]
        name: String,
    },
    /// Remove a project from wormhole (removes worktree for tasks)
    Remove {
        /// Project name
        #[arg(add = ArgValueCompleter::new(complete_projects))]
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
        #[arg(add = ArgValueCompleter::new(complete_projects))]
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

    /// Open a file, directory, project, or task
    Open {
        /// Path to file/directory, project name, or task (project:branch)
        #[arg(value_hint = ValueHint::AnyPath, add = ArgValueCompleter::new(complete_projects))]
        target: String,
        /// Which application to focus (only for project/task, not file/directory)
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
        #[arg(value_enum)]
        shell: Shell,
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
    /// Migrate worktrees from old layout ($branch) to new layout ($branch/$repo_name)
    MigrateWorktrees,
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

struct BranchCompleter {
    branches: Vec<String>,
}
impl Helper for BranchCompleter {}
impl Validator for BranchCompleter {}
impl Hinter for BranchCompleter {
    type Hint = String;
}
impl Highlighter for BranchCompleter {}
impl Completer for BranchCompleter {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let prefix = &line[..pos];
        let candidates: Vec<Pair> = self
            .branches
            .iter()
            .filter(|b| b.starts_with(prefix))
            .map(|b| Pair {
                display: b.clone(),
                replacement: b.clone(),
            })
            .collect();
        Ok((0, candidates))
    }
}

fn create_branch_editor(
    branches: Vec<String>,
) -> Result<Editor<BranchCompleter, rustyline::history::DefaultHistory>, String> {
    let config = Config::builder()
        .auto_add_history(false)
        .completion_type(rustyline::CompletionType::List)
        .build();
    let helper = BranchCompleter { branches };
    let mut rl: Editor<BranchCompleter, rustyline::history::DefaultHistory> =
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

fn parse_path_and_line(target: &str) -> (String, Option<usize>) {
    if let Some(idx) = target.rfind(':') {
        let (path, rest) = target.split_at(idx);
        if let Ok(line) = rest[1..].parse::<usize>() {
            if std::path::Path::new(path).exists() {
                return (path.to_string(), Some(line));
            }
        }
    }
    (target.to_string(), None)
}

fn build_query(land_in: &Option<String>, line: &Option<usize>) -> String {
    let mut params = vec![];
    if let Some(app) = land_in {
        params.push(format!("land-in={}", app));
    }
    if let Some(n) = line {
        params.push(format!("line={}", n));
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
            ProjectCommand::List {
                output,
                available,
                name_only,
                active,
            } => {
                let path = if active {
                    "/project/list?active=true"
                } else {
                    "/project/list"
                };
                let response = client.get(path)?;
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
                            if name_only {
                                if let Some(key) = item.get("project_key").and_then(|k| k.as_str())
                                {
                                    println!("{}", key);
                                }
                            } else {
                                println!("{}", render_project_item(item));
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

        Command::Open { target, land_in } => {
            let (path_str, line) = parse_path_and_line(&target);
            let target_path = std::path::Path::new(&path_str);

            if target_path.exists() {
                // File or directory - open in editor
                let abs_path = std::fs::canonicalize(target_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(path_str);
                let query = build_query(&Some("editor".to_string()), &line);
                let url_path = format!("/file/{}{}", abs_path, query);
                client.get(&url_path)?;
            } else {
                // Project name or task identifier - respects land-in KV
                let query = build_switch_query(&land_in, &None, &None, &None);
                let path = format!("/project/switch/{}{}", target, query);
                client.get(&path)?;
            }
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
            TaskCommand::Upsert {
                target,
                home_project,
            } => task_upsert(&client, &target, home_project),
            TaskCommand::CreateFromSprint => task_create_from_sprint(&client),
        },

        Command::Completion { shell } => {
            generate(shell, &mut Cli::command(), "wormhole", &mut io::stdout());
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
            DoctorCommand::MigrateWorktrees => doctor_migrate_worktrees(),
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
            let name_linked = ProjectKey::parse(&project.name).hyperlink();
            lines.push(format!("{}:", name_linked));
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

fn doctor_migrate_worktrees() -> Result<(), String> {
    let available = config::available_projects();
    let mut total = 0;
    for (name, path) in &available {
        if !crate::git::is_git_repo(path) {
            continue;
        }
        match crate::git::migrate_worktrees(name, path) {
            Ok(0) => {}
            Ok(n) => {
                println!("{}: migrated {} worktree(s)", name, n);
                total += n;
            }
            Err(e) => eprintln!("{}: error: {}", name, e),
        }
    }
    if total == 0 {
        println!("No worktrees needed migration.");
    } else {
        println!("\nMigrated {} worktree(s) total.", total);
    }
    Ok(())
}

// TODO: cli.rs is a large file. I think I'd prefer to create a cli directory, with the API defined
// in a single file, and then separate files for implementation of each top-level command, and
// shared utilities in a separate file.

fn task_create_from_sprint(client: &Client) -> Result<(), String> {
    use std::collections::HashMap;

    // Refresh to discover all existing worktrees before checking
    let _ = client.post("/project/refresh-tasks");

    let response = client.get("/project/list")?;
    let parsed: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    let current = parsed
        .get("current")
        .ok_or("Missing 'current' in /project/list response")?
        .as_array()
        .ok_or("'current' is not an array")?;

    // Build maps from the project list
    let mut existing_by_jira: HashMap<String, (String, String)> = HashMap::new();
    let mut has_non_draft_pr: HashMap<String, bool> = HashMap::new();

    for item in current {
        let project_key = item
            .get("project_key")
            .ok_or("Missing 'project_key' in project item")?
            .as_str()
            .ok_or("'project_key' is not a string")?;

        // Track PR status
        let pr_is_non_draft = item
            .get("pr")
            .is_some_and(|pr| !pr.get("isDraft").and_then(|d| d.as_bool()).unwrap_or(false));
        has_non_draft_pr.insert(project_key.to_string(), pr_is_non_draft);

        // Only process items with jira_key
        if let Some(jira_key) = item.get("kv").and_then(|kv| kv.get("jira_key")) {
            let jira_key = jira_key.as_str().ok_or("'jira_key' is not a string")?;
            let (repo, branch) = project_key.split_once(':').ok_or_else(|| {
                format!(
                    "Task with jira_key '{}' has invalid project_key '{}' (expected repo:branch)",
                    jira_key, project_key
                )
            })?;
            existing_by_jira.insert(jira_key.to_string(), (repo.to_string(), branch.to_string()));
        }
    }

    // Map (repo, branch) -> jira_key for reverse lookup
    let existing_by_task: HashMap<(String, String), String> = existing_by_jira
        .iter()
        .map(|(jira, (repo, branch))| ((repo.clone(), branch.clone()), jira.clone()))
        .collect();

    let available_projects = get_available_projects(client)?;
    let issues = jira::get_sprint_issues()?;

    let mut rl = create_project_editor(available_projects)?;
    let mut created_count = 0;
    let mut skipped_count = 0;

    for issue in &issues {
        let existing = existing_by_jira.get(&issue.key);

        // Check if task exists and has non-draft PR
        let has_pr = existing
            .map(|(repo, branch)| {
                let store_key = format!("{}:{}", repo, branch);
                *has_non_draft_pr.get(&store_key).unwrap_or(&false)
            })
            .unwrap_or(false);

        let indicator = jira::status_indicator(&issue.status);

        if let Some(reason) = should_skip_issue(has_pr) {
            println!("{} {} {} [{}]", indicator, issue.key, issue.summary, reason);
            skipped_count += 1;
            continue;
        }

        println!(
            "\n─────────────────────────────────────────────────────────────────────────────────"
        );
        println!(
            "{} {} {} [{}]",
            indicator, issue.key, issue.summary, issue.status
        );

        // If task exists locally, show it and offer to confirm/skip
        if let Some((existing_repo, existing_branch)) = existing {
            let existing_key = ProjectKey::task(existing_repo, existing_branch);
            println!("  Already exists locally: {}", existing_key.hyperlink());
            println!();
            let confirm = match rl.readline("  ▶ Keep existing? [Y/n/q]: ") {
                Ok(line) => line.trim().to_lowercase(),
                Err(rustyline::error::ReadlineError::Interrupted) => "n".to_string(),
                Err(rustyline::error::ReadlineError::Eof) => break,
                Err(e) => return Err(format!("Input error: {}", e)),
            };
            match confirm.as_str() {
                "" | "y" | "yes" => {
                    println!("  Keeping {}", existing_key.hyperlink());
                    skipped_count += 1;
                    continue;
                }
                "q" | "quit" => break,
                _ => {
                    println!("  Will prompt for new location (existing task will remain)");
                }
            }
        }

        // Prompt for home project
        let home = match rl.readline("  home: ") {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    eprintln!("  Skipping (no home)");
                    continue;
                }
                trimmed.to_string()
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("  Skipping");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => return Err(format!("Input error: {}", e)),
        };

        // Get branches from the selected repo for completion
        let branches = config::resolve_project_name(&home)
            .map(|path| crate::git::list_branches(&path))
            .unwrap_or_default();
        let mut branch_rl = create_branch_editor(branches)?;

        // Prompt for branch
        let default_branch = existing
            .map(|(_, branch)| branch.clone())
            .unwrap_or_else(|| to_kebab_case(&issue.summary));

        let branch = match branch_rl.readline_with_initial("  branch: ", (&default_branch, "")) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    default_branch
                } else {
                    trimmed.to_string()
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("  Skipping");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => return Err(format!("Input error: {}", e)),
        };

        // Safety check: warn if this repo:branch already has a different JIRA key
        let task_key_tuple = (home.clone(), branch.clone());
        if let Some(other_jira) = existing_by_task.get(&task_key_tuple) {
            if other_jira != &issue.key {
                let key = ProjectKey::task(&home, &branch);
                eprintln!(
                    "  WARNING: {} is already linked to {}",
                    key.hyperlink(),
                    other_jira
                );
                let confirm = match rl.readline("  Continue anyway? [y/N]: ") {
                    Ok(line) => line.trim().to_lowercase(),
                    Err(_) => "n".to_string(),
                };
                if confirm != "y" && confirm != "yes" {
                    eprintln!("  Skipping to avoid conflict");
                    continue;
                }
            }
        }

        // Safety check: warn if changing repo or branch for existing JIRA task
        if let Some((existing_repo, existing_branch)) = existing {
            if &home != existing_repo || &branch != existing_branch {
                let existing_key = ProjectKey::task(existing_repo, existing_branch);
                let new_key = ProjectKey::task(&home, &branch);
                eprintln!(
                    "  WARNING: {} already exists as {}",
                    issue.key,
                    existing_key.hyperlink()
                );
                eprintln!(
                    "  Creating {} will result in duplicate tasks for same JIRA",
                    new_key.hyperlink()
                );
                let confirm = match rl.readline("  Continue anyway? [y/N]: ") {
                    Ok(line) => line.trim().to_lowercase(),
                    Err(_) => "n".to_string(),
                };
                if confirm != "y" && confirm != "yes" {
                    eprintln!("  Skipping to avoid duplicate");
                    continue;
                }
            }
        }

        // Final confirmation before creating
        let task_key = ProjectKey::task(&home, &branch);
        println!("  Creating {} for {}", task_key.hyperlink(), issue.key);
        upsert_task(client, &home, &branch, Some(&issue.key))?;
        println!("  Created {}", task_key.hyperlink());
        created_count += 1;
    }

    println!(
        "\nDone: {} created, {} skipped",
        created_count, skipped_count
    );

    if created_count > 0 {
        let _ = client.post("/project/refresh");
    }

    Ok(())
}

/// Represents a parsed task target for the upsert command
enum UpsertTarget {
    /// A project key like "repo:branch"
    ProjectKey { repo: String, branch: String },
    /// A JIRA key (bare like "ACT-123" or extracted from URL)
    JiraKey(String),
}

/// Find an existing task by JIRA key from the project list
fn find_task_by_jira_key(
    client: &Client,
    jira_key: &str,
) -> Result<Option<(String, String)>, String> {
    let response = client.get("/project/list")?;
    let parsed: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;

    if let Some(current) = parsed.get("current").and_then(|v| v.as_array()) {
        for item in current {
            if let Some(kv_jira) = item
                .get("kv")
                .and_then(|kv| kv.get("jira_key"))
                .and_then(|k| k.as_str())
            {
                if kv_jira == jira_key {
                    if let Some(project_key) = item.get("project_key").and_then(|k| k.as_str()) {
                        if let Some((repo, branch)) = project_key.split_once(':') {
                            return Ok(Some((repo.to_string(), branch.to_string())));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

/// Get existing task info by project key
fn get_task_info(client: &Client, repo: &str, branch: &str) -> Result<Option<String>, String> {
    let store_key = format!("{}:{}", repo, branch);
    let kv_url = format!("/kv/{}/jira_key", store_key);
    match client.get(&kv_url) {
        Ok(jira_key) => Ok(Some(jira_key)),
        Err(_) => Ok(None),
    }
}

fn task_upsert(client: &Client, target: &str, home_project: Option<String>) -> Result<(), String> {
    // Refresh to get latest task list
    let _ = client.post("/project/refresh-tasks");

    // Parse target to determine what we're working with
    let (upsert_target, existing_task) = parse_upsert_target(client, target)?;

    // Get JIRA info if we have a JIRA key
    let (jira_key, jira_issue) = match &upsert_target {
        UpsertTarget::JiraKey(key) => {
            let issue = jira::get_issue(key)?;
            (Some(key.clone()), issue)
        }
        UpsertTarget::ProjectKey { repo, branch } => {
            // Check if existing task has a JIRA key
            match get_task_info(client, repo, branch)? {
                Some(key) => {
                    let issue = jira::get_issue(&key)?;
                    (Some(key), issue)
                }
                None => (None, None),
            }
        }
    };

    // Print header with JIRA info if available
    if let Some(ref key) = jira_key {
        if let Some(ref issue) = jira_issue {
            println!("{} {}", key, issue.summary);
        } else {
            println!("{}", key);
        }
    }

    // Determine defaults
    let (default_home, default_branch) = match (&existing_task, &upsert_target) {
        (Some((repo, branch)), _) => (repo.clone(), branch.clone()),
        (None, UpsertTarget::ProjectKey { repo, branch }) => (repo.clone(), branch.clone()),
        (None, UpsertTarget::JiraKey(_)) => {
            let home = home_project
                .clone()
                .or_else(|| std::env::var("WORMHOLE_DEFAULT_HOME_PROJECT").ok())
                .unwrap_or_default();
            let branch = jira_issue
                .as_ref()
                .map(|i| to_kebab_case(&i.summary))
                .unwrap_or_default();
            (home, branch)
        }
    };

    let available_projects = get_available_projects(client)?;
    let mut rl = create_project_editor(available_projects)?;

    // Prompt for home project
    let home = if let Some(h) = home_project {
        h
    } else {
        match rl.readline_with_initial("home: ", (&default_home, "")) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    if default_home.is_empty() {
                        return Err("Home project is required".to_string());
                    }
                    default_home.clone()
                } else {
                    trimmed.to_string()
                }
            }
            Err(_) => return Err("Aborted".to_string()),
        }
    };

    // Get branches from the selected repo for completion
    let branches = config::resolve_project_name(&home)
        .map(|path| crate::git::list_branches(&path))
        .unwrap_or_default();
    let mut branch_rl = create_branch_editor(branches)?;

    // Prompt for branch
    let branch = match branch_rl.readline_with_initial("branch: ", (&default_branch, "")) {
        Ok(line) => {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if default_branch.is_empty() {
                    return Err("Branch is required".to_string());
                }
                default_branch.clone()
            } else {
                trimmed.to_string()
            }
        }
        Err(_) => return Err("Aborted".to_string()),
    };

    let task_key = ProjectKey::task(&home, &branch);
    let same_location = existing_task
        .as_ref()
        .is_some_and(|(r, b)| r == &home && b == &branch);
    let is_move = existing_task.is_some() && !same_location;

    if same_location {
        println!("Updating {}", task_key.hyperlink());
    } else if is_move {
        let (old_repo, old_branch) = existing_task.as_ref().unwrap();
        let old_key = ProjectKey::task(old_repo, old_branch);
        println!("Moving {} → {}", old_key.hyperlink(), task_key.hyperlink());
    } else {
        println!("Creating {}", task_key.hyperlink());
    }

    // Create/ensure new task exists
    upsert_task(client, &home, &branch, jira_key.as_deref())?;

    // Delete old worktree if moving to a new location
    if is_move {
        let (old_repo, old_branch) = existing_task.as_ref().unwrap();
        let old_key = format!("{}:{}", old_repo, old_branch);
        if let Err(e) = client.post(&format!("/project/remove/{}", old_key)) {
            eprintln!("Warning: failed to remove old worktree: {}", e);
        }
    }

    // Refresh cache
    let _ = client.post("/project/refresh");

    if same_location {
        println!("Updated {}", task_key.hyperlink());
    } else if is_move {
        println!("Moved to {}", task_key.hyperlink());
    } else if let Some(ref key) = jira_key {
        println!("Created task {} for {}", task_key.hyperlink(), key);
    } else {
        println!("Created task {}", task_key.hyperlink());
    }

    Ok(())
}

fn parse_upsert_target(
    client: &Client,
    target: &str,
) -> Result<(UpsertTarget, Option<(String, String)>), String> {
    // First, check if it's a JIRA URL or key
    if let Some(jira_key) = crate::describe::parse_jira_key_or_url(target) {
        let existing = find_task_by_jira_key(client, &jira_key)?;
        return Ok((UpsertTarget::JiraKey(jira_key), existing));
    }

    // Check if it's a project key (repo:branch)
    if let Some((repo, branch)) = target.split_once(':') {
        // Verify the task exists (or at least the repo exists)
        let existing = if client
            .get(&format!("/kv/{}:{}/jira_key", repo, branch))
            .is_ok()
        {
            Some((repo.to_string(), branch.to_string()))
        } else {
            None
        };
        return Ok((
            UpsertTarget::ProjectKey {
                repo: repo.to_string(),
                branch: branch.to_string(),
            },
            existing,
        ));
    }

    Err(format!(
        "Could not parse target '{}'. Expected: project key (repo:branch), JIRA URL, or JIRA key (ACT-123)",
        target
    ))
}

/// Create or update a task with optional JIRA key
fn upsert_task(
    client: &Client,
    home: &str,
    branch: &str,
    jira_key: Option<&str>,
) -> Result<(), String> {
    // Create the worktree/task
    let url = format!("/project/create/{}?home-project={}", branch, home);
    client.get(&url)?;

    // Store JIRA key if provided
    if let Some(key) = jira_key {
        let store_key = format!("{}:{}", home, branch);
        let kv_url = format!("/kv/{}/jira_key", store_key);
        let _ = client.put(&kv_url, key);
    }

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
    let mut sprint_tasks: Vec<&serde_json::Value> = json
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
    sprint_tasks.sort_by_key(|item| {
        let status = item
            .get("jira")
            .and_then(|j| j.get("status"))
            .and_then(|s| s.as_str());
        status_sort_order(status)
    });

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

fn status_sort_order(status: Option<&str>) -> u8 {
    match status.map(|s| s.to_lowercase()).as_deref() {
        Some("done") | Some("closed") | Some("resolved") => 0,
        Some("in review") => 1,
        Some("in progress") => 2,
        Some("to do") => 3,
        _ => 4,
    }
}

/// Render a project item from /project/list response
fn render_project_item(item: &serde_json::Value) -> String {
    let project_key_str = item
        .get("project_key")
        .and_then(|k| k.as_str())
        .unwrap_or("");
    let task_display = ProjectKey::parse(project_key_str).hyperlink();

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

    let indicator = jira::status_indicator(status);
    let pad = 40_usize.saturating_sub(project_key_str.len());

    format!(
        "{} {}{} {}{}",
        indicator,
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
    let project_key = match &status.branch {
        Some(branch) => ProjectKey::task(&status.name, branch),
        None => ProjectKey::project(&status.name),
    };
    let name_linked = project_key.hyperlink();
    let name_display = project_key.to_string();

    let title = if let Some(ref jira) = status.jira {
        format!("{}: {}", name_linked, jira.summary)
    } else {
        name_linked.clone()
    };
    let title_len = if let Some(ref jira) = status.jira {
        name_display.len() + 2 + jira.summary.len()
    } else {
        name_display.len()
    };

    let mut lines = vec![title, "─".repeat(title_len)];

    if let Some(ref branch) = status.branch {
        lines.push(format!("Branch:    {}", branch));
    }

    if let Some(ref jira) = status.jira {
        lines.push(format!(
            "JIRA:      {} {}",
            crate::jira::status_indicator(&jira.status),
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
    let jira_instance = std::env::var("JIRA_INSTANCE").ok();
    let key_display = if let Some(ref instance) = jira_instance {
        let url = format!("https://{}.atlassian.net/browse/{}", instance, issue.key);
        crate::format_osc8_hyperlink(&url, &issue.key)
    } else {
        issue.key.clone()
    };
    format!(
        "{} {}: {}",
        crate::jira::status_indicator(&issue.status),
        key_display,
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

/// Determines if we should auto-skip this issue (no prompt).
/// Returns None if we should prompt, or Some(reason) if we should skip.
/// Only skips issues that have a non-draft PR (work is already submitted).
fn should_skip_issue(has_pr: bool) -> Option<&'static str> {
    if has_pr {
        return Some("has PR");
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
    fn test_should_skip_issues_with_pr() {
        assert_eq!(should_skip_issue(true), Some("has PR"));
    }

    #[test]
    fn test_should_not_skip_issues_without_pr() {
        assert_eq!(should_skip_issue(false), None);
    }

    #[test]
    fn test_render_project_item_bare_project() {
        // Project without JIRA - just shows project_key
        let item = serde_json::json!({
            "project_key": "wormhole",
            "path": "/Users/dan/src/wormhole"
        });
        let rendered = render_project_item(&item);
        // Should contain the project key (inside a hyperlink)
        assert!(rendered.contains("wormhole"), "Should contain project key");
        // Should not contain emoji (no JIRA)
        assert!(
            !rendered.contains("●"),
            "Should not have indicator without JIRA"
        );
    }

    #[test]
    fn test_render_project_item_task_without_jira() {
        // Task but no JIRA - shows project_key
        let item = serde_json::json!({
            "project_key": "cli:feature-branch",
            "path": "/Users/dan/src/cli/feature-branch"
        });
        let rendered = render_project_item(&item);
        assert!(
            rendered.contains("cli:feature-branch"),
            "Should show project_key"
        );
        assert!(
            !rendered.contains("●"),
            "Should not have indicator without JIRA"
        );
    }

    #[test]
    fn test_render_project_item_task_with_jira() {
        // Task with JIRA info - shows emoji, task, JIRA key (no summary)
        let item = serde_json::json!({
            "project_key": "cli:standalone-activity",
            "path": "/Users/dan/src/cli/standalone-activity",
            "jira": {
                "key": "ACT-107",
                "status": "In Progress",
                "summary": "Standalone activity CLI integration"
            }
        });
        let rendered = render_project_item(&item);
        assert!(rendered.contains("●"), "Should contain status indicator");
        assert!(
            rendered.contains("cli:standalone-activity"),
            "Should contain project_key"
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
            "project_key": "cli:feature",
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
            "project_key": "cli:feature",
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
