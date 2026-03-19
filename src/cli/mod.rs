use clap::builder::ValueHint;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::engine::{
    ArgValueCompleter, CompletionCandidate, PathCompleter, ValueCompleter,
};
use clap_complete::{generate, Shell};
use std::io;

use crate::config;
use crate::tty::TerminalHyperlink;

mod doctor;
mod jira;
pub mod project;
mod task;
mod util;
use project::ProjectDebug;
use util::*;

fn expand_tilde(input: &std::ffi::OsStr) -> Option<std::ffi::OsString> {
    let s = input.to_str()?;
    if s == "~" {
        Some(std::env::var_os("HOME")?)
    } else {
        let rest = s.strip_prefix("~/")?;
        let mut home = std::env::var_os("HOME")?;
        home.push("/");
        home.push(rest);
        Some(home)
    }
}

fn complete_projects(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let expanded = expand_tilde(current);
    let mut candidates = PathCompleter::any().complete(expanded.as_deref().unwrap_or(current));

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
    Create {
        /// Target: project key (repo:branch), JIRA URL, or JIRA key (ACT-123)
        #[arg(add = ArgValueCompleter::new(complete_projects))]
        target: String,
        /// Home project for the worktree (required for create)
        #[arg(short = 'p', long, add = ArgValueCompleter::new(complete_projects))]
        home_project: Option<String>,
    },
    /// Create tasks from current sprint issues
    CreateFromSprint,
    /// Create tasks from GitHub PRs requesting your review
    CreateFromReviewRequests {
        /// Show what would be created without actually creating tasks
        #[arg(long)]
        dry_run: bool,
    },
    /// Create a task from a GitHub issue
    CreateFromIssue {
        /// GitHub issue URL or owner/repo#123
        target: String,
        /// Home project for the worktree
        #[arg(short = 'p', long)]
        home_project: Option<String>,
        /// Show what would be created without actually creating
        #[arg(long)]
        dry_run: bool,
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
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// List projects (current and available)
    List {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
        /// List available projects (from search_paths config) instead of current
        #[arg(short, long)]
        available: bool,
        /// Output only project names (for shell completion)
        #[arg(long)]
        name_only: bool,
        /// List only projects with a tmux window
        #[arg(long)]
        active: bool,
        /// List only tasks (projects with a branch)
        #[arg(long)]
        tasks: bool,
        /// List only projects with an editor connected
        #[arg(long)]
        with_editor: bool,
        /// Filter by JIRA status (e.g. "Done", "In Progress")
        #[arg(long)]
        status: Option<String>,
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
        /// Project name (defaults to current project)
        #[arg(add = ArgValueCompleter::new(complete_projects))]
        name: Option<String>,
    },
    /// Close all open projects
    CloseAll,
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
    /// Send a JSON-RPC message to a project's extension
    Message {
        /// Project name (defaults to current project)
        #[arg(add = ArgValueCompleter::new(complete_projects))]
        name: Option<String>,
        /// JSON-RPC method (e.g. editor/close)
        #[arg(short, long)]
        method: String,
        /// Target role (default: editor)
        #[arg(short, long, default_value = "editor")]
        target: String,
    },
    /// Run a command in each project directory
    ForEach {
        /// Only run on tasks (not plain repos)
        #[arg(long)]
        tasks: bool,
        /// Only run on projects with a tmux window
        #[arg(long)]
        active: bool,
        /// Show status of running/recent batches
        #[arg(long)]
        status: bool,
        /// Cancel a running batch
        #[arg(long)]
        cancel: Option<String>,
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
        /// Show progress on stderr
        #[arg(short, long)]
        verbose: bool,
        /// Command to run in each project directory
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
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

    /// Agent conversation operations
    Conversations {
        #[command(subcommand)]
        command: ConversationsCommand,
    },

    /// Refresh in-memory data from external sources
    Refresh,
}

#[derive(Subcommand)]
pub enum ConversationsCommand {
    /// Sync agent conversations to searchable text files
    Sync {
        /// Only sync conversations for these projects (repeatable)
        #[arg(short, long, add = ArgValueCompleter::new(complete_projects))]
        project: Vec<String>,
        /// Only include conversations newer than this duration (e.g. 2w, 3d, 1m)
        #[arg(short, long)]
        since: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum DoctorCommand {
    /// Report on persisted wormhole data (worktrees, KV files on disk)
    PersistedData {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
    },
    /// List open editor windows
    ListEditorWindows {
        /// Output format: text (default) or json
        #[arg(short, long, default_value = "text")]
        output: String,
    },
    /// Conform task worktrees to desired state (.task/AGENTS.md, symlinks, etc.)
    Conform {
        /// Show what would be done without making changes
        #[arg(long)]
        dry_run: bool,
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

fn is_conversation_file(path: &std::path::Path) -> bool {
    let conversations_dir = std::fs::canonicalize(crate::conversations::conversations_dir())
        .unwrap_or_else(|_| crate::conversations::conversations_dir());
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    abs_path.starts_with(&conversations_dir)
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
                tasks,
                with_editor,
                status,
            } => {
                let mut query_parts = vec![];
                if active {
                    query_parts.push("active=true");
                }
                if tasks {
                    query_parts.push("tasks=true");
                }
                if with_editor {
                    query_parts.push("with-editor=true");
                }
                let path = if query_parts.is_empty() {
                    "/project/list".to_string()
                } else {
                    format!("/project/list?{}", query_parts.join("&"))
                };
                let response = client.get(&path)?;
                if output == "json" {
                    println!("{}", response);
                } else if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                    if available {
                        if let Some(avail) = json.get("available").and_then(|v| v.as_array()) {
                            for item in avail {
                                if let Some(name) = item.as_str() {
                                    println!(
                                        "{}",
                                        crate::project::ProjectKey::parse(name).hyperlink()
                                    );
                                }
                            }
                        }
                    } else if let Some(current) = json.get("current").and_then(|v| v.as_array()) {
                        let mut sorted = current.clone();
                        if let Some(ref s) = status {
                            let s_lower = s.to_lowercase();
                            sorted.retain(|item| {
                                item.get("jira")
                                    .and_then(|j| j.get("status"))
                                    .and_then(|v| v.as_str())
                                    .is_some_and(|st| st.to_lowercase() == s_lower)
                            });
                        }
                        sorted.sort_by(|a, b| {
                            let a_key = a.get("project_key").and_then(|k| k.as_str()).unwrap_or("");
                            let b_key = b.get("project_key").and_then(|k| k.as_str()).unwrap_or("");
                            a_key.cmp(b_key)
                        });
                        for item in &sorted {
                            if name_only {
                                if let Some(key) = item.get("project_key").and_then(|k| k.as_str())
                                {
                                    println!("{}", key);
                                }
                            } else {
                                println!("{}", project::render_project_item(item));
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
                let name = name.unwrap_or_else(|| {
                    std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
                client.post(&format!("/project/close/{}", name))?;
                Ok(())
            }
            ProjectCommand::CloseAll => {
                client.post("/project/close-all")?;
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
            ProjectCommand::ForEach {
                tasks,
                active,
                status,
                cancel,
                output,
                verbose,
                command,
            } => project::for_each(
                &client, tasks, active, status, cancel, &command, &output, verbose,
            ),
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
                    println!("{}", project::render_task_status(&status));
                }
                Ok(())
            }
            ProjectCommand::Message {
                name,
                method,
                target,
            } => {
                let name = name.unwrap_or_else(|| {
                    std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
                let body = serde_json::json!({
                    "target": target,
                    "message": {
                        "jsonrpc": "2.0",
                        "method": method,
                    }
                });
                client.post_json(&format!("/project/messages/{}", name), &body)?;
                Ok(())
            }
        },

        Command::Open { target, land_in } => {
            let (path_str, line) = parse_path_and_line(&target);
            let target_path = std::path::Path::new(&path_str);

            if target_path.is_file() && is_conversation_file(target_path) {
                let abs_path = std::fs::canonicalize(target_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(path_str);
                client.post(&format!("/conversations/resume/{}", abs_path))?;
            } else if target_path.is_file() {
                let abs_path = std::fs::canonicalize(target_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(path_str);
                let query = build_query(&Some("editor".to_string()), &line);
                let url_path = format!("/file/{}{}", abs_path, query);
                client.get(&url_path)?;
            } else if target_path.is_dir() {
                // Directory - switch to project
                let abs_path = std::fs::canonicalize(target_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(path_str);
                let query = build_switch_query(&land_in, &None, &None, &None);
                let path = format!("/project/switch/{}{}", abs_path, query);
                client.get(&path)?;
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
                let kv = project::KvValue {
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
                None => jira::sprint_list(&client, "text"),
                Some(SprintCommand::List { output }) => jira::sprint_list(&client, &output),
                Some(SprintCommand::Show { output }) => jira::sprint_show(&client, &output),
            },
        },

        Command::Task { command } => match command {
            TaskCommand::Create {
                target,
                home_project,
            } => task::task_create(&client, &target, home_project),
            TaskCommand::CreateFromSprint => task::task_create_from_sprint(&client),
            TaskCommand::CreateFromReviewRequests { dry_run } => {
                task::task_create_from_review_requests(&client, dry_run)
            }
            TaskCommand::CreateFromIssue {
                target,
                home_project,
                dry_run,
            } => task::task_create_from_issue(&client, &target, home_project.as_deref(), dry_run),
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
            DoctorCommand::PersistedData { output } => {
                doctor::doctor_persisted_data(&client, &output)
            }
            DoctorCommand::ListEditorWindows { output } => {
                doctor::doctor_list_editor_windows(&client, &output)
            }
            DoctorCommand::Conform { dry_run, output } => {
                doctor::doctor_conform(&client, dry_run, &output)
            }
        },

        Command::Conversations { command } => match command {
            ConversationsCommand::Sync { project, since } => {
                let mut params = Vec::new();
                if !project.is_empty() {
                    params.push(format!("project={}", project.join(",")));
                }
                if let Some(ref s) = since {
                    params.push(format!("since={}", s));
                }
                let query = if params.is_empty() {
                    String::new()
                } else {
                    format!("?{}", params.join("&"))
                };
                let response = client.post(&format!("/conversations/sync{}", query))?;
                let result: crate::conversations::SyncResult =
                    serde_json::from_str(&response).map_err(|e| e.to_string())?;
                eprintln!(
                    "Synced {} conversations ({} unchanged)",
                    result.synced, result.skipped
                );
                println!("{}", result.output_dir);
                Ok(())
            }
        },

        Command::Refresh => {
            client.post("/project/refresh")?;
            println!("Refreshed");
            Ok(())
        }
    }
}
