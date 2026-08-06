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
        let cwd = project.checked_working_tree()?;
        let vars = shell_env_vars(project);
        let window_id = tmux_vec(vec![
            "new-window".to_string(),
            "-n".to_string(),
            window_name.clone(),
            "-c".to_string(),
            cwd.to_string_lossy().to_string(),
            "-P".to_string(),
            "-F".to_string(),
            "#{window_id}".to_string(),
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
        // Target by window id, not name: a task's store_key contains a ':',
        // which tmux would otherwise parse as a session:window target.
        tmux([
            "set-option",
            "-w",
            "-t",
            window_id.trim(),
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

/// Run `cmd` in a pane of its own in the project's window, started in `cwd`,
/// and focus it.
///
/// What the command is, and which directory it has to be run in, are the
/// caller's business: a program that files its state by directory can only be
/// picked up where it was left, and this has no way of knowing that. It obeys.
///
/// Always a new pane. Whether a pane still has something running in it cannot
/// be told from outside with any confidence — an interactive shell here keeps
/// helpers of its own in the foreground, which read exactly like a program that
/// has not finished — and a pane guessed wrong about is either typed into while
/// somebody is working in it or focused while nothing is. Whether the thing is
/// already running is known to the caller, who says so with a pid, and that
/// pane is focused instead of this being called at all.
pub fn run_in_pane(project: &Project, cwd: &str, cmd: &str) {
    let _ = open(project);
    let window = match get_window(&project.store_key().to_string()) {
        Some(w) => w,
        None => return,
    };
    let Some(pane_id) = split_pane(&window.id, cwd) else {
        return;
    };
    tmux(["send-keys", "-t", &pane_id, cmd, "Enter"]);
    tmux(["select-window", "-t", &window.id]);
    tmux(["select-pane", "-t", &pane_id]);
}

fn split_pane(window_id: &str, cwd: &str) -> Option<String> {
    let pane_id = tmux_vec(vec![
        "split-window".to_string(),
        "-t".to_string(),
        window_id.to_string(),
        "-c".to_string(),
        cwd.to_string(),
        "-P".to_string(),
        "-F".to_string(),
        "#{pane_id}".to_string(),
    ]);
    let pane_id = pane_id.trim();
    (!pane_id.is_empty()).then(|| pane_id.to_string())
}

/// Focus the pane a process is running in. Returns false when it is not in one.
///
/// A session started by hand lives in a pane wormhole never tagged, so looking
/// for the tag is not enough: found by the controlling terminal of the process
/// itself, which is the pane's tty.
pub fn focus_pane_with_pid(pid: u32) -> bool {
    let Some(tty) = tty_of(pid) else {
        return false;
    };
    let fmt = "#{pane_id} #{window_id} #{pane_tty}";
    for line in tmux(["list-panes", "-a", "-F", fmt]).lines() {
        let mut fields = line.split_whitespace();
        let (Some(pane), Some(window), Some(pane_tty)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if pane_tty.trim_start_matches("/dev/") == tty {
            tmux(["select-window", "-t", window]);
            tmux(["select-pane", "-t", pane]);
            return true;
        }
    }
    false
}

fn tty_of(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-o", "tty=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let tty = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!tty.is_empty() && tty != "??").then_some(tty)
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
