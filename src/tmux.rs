use core::str;
use std::collections::HashMap;
use std::process::Command;
use std::thread;

use crate::project::Project;
use crate::terminal::write_wormhole_env_vars;
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
                let common_prefix = existing_directory
                    .chars()
                    .zip(directory.chars())
                    .take_while(|(a, b)| a == b)
                    .map(|(a, _)| a)
                    .collect();
                directories.insert(window_name, common_prefix);
            } else {
                directories.insert(window_name, directory);
            }
        });
    directories.into_values().collect()
}

pub fn window_names() -> Vec<String> {
    list_windows().into_iter().map(|w| w.name).collect()
}

pub fn exists(project: &Project) -> bool {
    get_window(&project.name).is_some()
}

pub fn open(project: &Project) -> Result<(), String> {
    if let Some(window) = get_window(&project.name) {
        tmux(["select-window", "-t", &window.id]);
    } else {
        tmux([
            "new-window",
            "-n",
            &project.name,
            "-c",
            project.path.to_str().unwrap(),
        ]);
    }
    let project = project.clone();
    thread::spawn(move || write_wormhole_env_vars(&project));
    Ok(())
}

pub fn close(project: &Project) {
    if let Some(window) = get_window(&project.name) {
        tmux(["kill-window", "-t", &window.id]);
    }
}

fn get_window(name: &str) -> Option<Window> {
    for w in list_windows() {
        if w.name == name {
            return Some(w);
        }
    }
    None
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
    // TODO: once
    // E.g. TMUX=/private/tmp/tmux-501/default,89323,0
    let socket_path = std::env::var("TMUX")
        .unwrap_or_else(|_| panic("TMUX env var is not set"))
        .split(",")
        .nth(0)
        .unwrap()
        .to_string();

    let program = "tmux";
    let output = Command::new(program)
        .args(["-S", &socket_path])
        .args(args)
        .output()
        .unwrap_or_else(|_| panic("Failed to execute command"));
    get_stdout(program, output)
}
