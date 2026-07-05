use core::str;
use std::collections::HashMap;
use std::process::Command;

use crate::project::Project;
use crate::terminal::shell_env_vars;
use crate::util::{get_stdout, panic};

struct Window {
    id: String,
    name: String,
}

/// Return a directory for each window based on the common prefix of the directories
/// of its panes.
// TODO: this effectively makes the terminal windows the point of truth for
// 'current projects'. However, the current logic will break if:
// - a window pane's directory lies outside the project root
// - all panes of a window are not in non-root subdirectories of the project
pub fn project_directories() -> Vec<String> {
    let mut directories = HashMap::<String, String>::new();
    tmux(["list-panes", "-a", "-F", "#W #{pane_current_path}"])
        .split_terminator("\n")
        .for_each(|line| {
            let mut fields = line.split(" ");
            let window_name = fields.next().unwrap().to_string();
            let directory = fields.next().unwrap().to_string();
            if let Some(existing_directory) = directories.get(&window_name) {
                let common_prefix: String = existing_directory
                    .chars()
                    .zip(directory.chars())
                    .take_while(|(a, b)| a == b)
                    .map(|(a, _)| a)
                    .collect();
                // If the common prefix is just "/", skip this window as it has
                // disparate pane directories with no meaningful common root
                if common_prefix != "/" {
                    directories.insert(window_name, common_prefix);
                }
            } else {
                directories.insert(window_name, directory);
            }
        });
    directories
        .into_values()
        .filter(|dir| dir != "/") // Also filter out any remaining root directories
        .collect()
}

pub fn window_names() -> Vec<String> {
    list_windows().into_iter().map(|w| w.name).collect()
}

pub fn exists(project: &Project) -> bool {
    get_window(&project.store_key().to_string()).is_some()
}

pub fn open(project: &Project) -> Result<(), String> {
    let window_name = project.store_key().to_string();
    if let Some(window) = get_window(&window_name) {
        tmux(["select-window", "-t", &window.id]);
    } else {
        let vars = shell_env_vars(project);
        tmux_vec(vec![
            "new-window".to_string(),
            "-n".to_string(),
            window_name.clone(),
            "-c".to_string(),
            project.working_tree().to_string_lossy().to_string(),
            "-e".to_string(),
            format!("WORMHOLE_PROJECT_NAME={}", vars.project_name),
            "-e".to_string(),
            format!("WORMHOLE_PROJECT_DIR={}", vars.project_dir),
            "-e".to_string(),
            format!("WORMHOLE_JIRA_URL={}", vars.jira_url),
            "-e".to_string(),
            format!("WORMHOLE_GITHUB_REPO={}", vars.github_repo),
            "-e".to_string(),
            format!("WORMHOLE_GITHUB_PR_URL={}", vars.github_pr_url),
        ]);
        // Tag the project window with the generic @project key so auxiliary
        // windows (e.g. tide's browsers) can be associated and reaped together.
        tmux([
            "set-option",
            "-w",
            "-t",
            &window_name,
            "@project",
            &window_name,
        ]);
    }
    Ok(())
}

pub fn close(project: &Project) {
    let store_key = project.store_key().to_string();
    // The main project window (matched by name) plus any auxiliary windows
    // tagged with this project (e.g. tide's browsers). Both are collected as
    // stable window ids and deduped, so each window is killed exactly once
    // even when the main window is itself @project-tagged.
    for id in project_window_ids(&store_key) {
        tmux(["kill-window", "-t", &id]);
    }
}

fn project_window_ids(store_key: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let listing = tmux([
        "list-windows",
        "-a",
        "-F",
        "#{window_id}\t#{window_name}\t#{@project}",
    ]);
    for line in listing.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 2 {
            continue;
        }
        let (id, name) = (fields[0], fields[1]);
        let project = fields.get(2).copied().unwrap_or("");
        if (name == store_key || project == store_key) && !ids.iter().any(|x| x == id) {
            ids.push(id.to_string());
        }
    }
    ids
}

/// Open (or focus) a tmux pane running `claude -r <session_id>` in the project's
/// window. A pane already running this session is reused; otherwise a new pane
/// is split off, tagged with the session id, and `claude -r` is launched in it.
pub fn resume_claude_session(project: &Project, session_id: &str) {
    let _ = open(project);
    let window = match get_window(&project.store_key().to_string()) {
        Some(w) => w,
        None => return,
    };
    if let Some(pane_id) = find_session_pane(&window.id, session_id) {
        tmux(["select-window", "-t", &window.id]);
        tmux(["select-pane", "-t", &pane_id]);
        return;
    }
    let dir = project.working_tree().to_string_lossy().to_string();
    let pane_id = tmux_vec(vec![
        "split-window".to_string(),
        "-t".to_string(),
        window.id.clone(),
        "-c".to_string(),
        dir,
        "-P".to_string(),
        "-F".to_string(),
        "#{pane_id}".to_string(),
    ]);
    let pane_id = pane_id.trim();
    if pane_id.is_empty() {
        return;
    }
    tmux([
        "set-option",
        "-p",
        "-t",
        pane_id,
        SESSION_PANE_OPTION,
        session_id,
    ]);
    let cmd = format!("claude -r {session_id}");
    tmux(["send-keys", "-t", pane_id, cmd.as_str(), "Enter"]);
    tmux(["select-window", "-t", &window.id]);
    tmux(["select-pane", "-t", pane_id]);
}

const SESSION_PANE_OPTION: &str = "@wormhole_claude_session";

fn find_session_pane(window_id: &str, session_id: &str) -> Option<String> {
    let fmt = format!("#{{pane_id}} #{{{SESSION_PANE_OPTION}}}");
    tmux(["list-panes", "-t", window_id, "-F", fmt.as_str()])
        .lines()
        .find_map(|line| {
            let (pane, sess) = line.split_once(' ')?;
            (sess == session_id).then(|| pane.to_string())
        })
}

fn get_window(name: &str) -> Option<Window> {
    list_windows().into_iter().find(|w| w.name == name)
}

fn list_windows() -> Vec<Window> {
    tmux(["list-windows", "-F", "#I #W"])
        .split_terminator("\n")
        .map(|line| {
            let mut fields = line.split(" ");
            Window {
                id: fields.next().unwrap().to_string(),
                name: fields.next().unwrap().to_string(),
            }
        })
        .collect()
}

pub fn tmux<'a, I>(args: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    tmux_vec(args.into_iter().map(|s| s.to_string()).collect())
}

fn tmux_vec(args: Vec<String>) -> String {
    let socket_path = std::env::var("WORMHOLE_TMUX")
        .or_else(|_| std::env::var("TMUX"))
        .unwrap_or_else(|_| panic("TMUX env var is not set"))
        .split(",")
        .next()
        .unwrap()
        .to_string();

    let program = "tmux";
    let output = Command::new(program)
        .args(["-S", &socket_path])
        .args(&args)
        .output()
        .unwrap_or_else(|_| panic("Failed to execute command"));
    get_stdout(program, output).unwrap_or_else(|e| panic(&e))
}
