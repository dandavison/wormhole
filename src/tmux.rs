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
            window_name,
            "-c".to_string(),
            project.working_dir().to_string_lossy().to_string(),
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
    }
    Ok(())
}

pub fn close(project: &Project) {
    if let Some(window) = get_window(&project.store_key().to_string()) {
        tmux(["kill-window", "-t", &window.id]);
    }
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
    get_stdout(program, output)
}
