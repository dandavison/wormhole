use std::fs;
use std::str;
use std::thread;
use std::{process::Command, slice::Iter};

use crate::config;
use crate::hammerspoon;
use crate::project::Project;

struct Window {
    id: String,
    name: String,
}

pub fn open(project: &Project) -> Result<(), String> {
    if let Some(window) = get_window(&project.name) {
        tmux(["select-window", "-t", &window.id].iter());
    } else {
        tmux(
            [
                "new-window",
                "-n",
                &project.name,
                "-c",
                project.path.to_str().unwrap(),
            ]
            .iter(),
        );
    }
    let project = project.clone();
    thread::spawn(move || {
        fs::write(
            config::ENV_FILE,
            format!(
                "export WORMHOLE_PROJECT_NAME={} WORMHOLE_PROJECT_DIR={}",
                &project.name,
                project.path.as_path().to_str().unwrap()
            ),
        )
        .unwrap()
    });
    Ok(())
}

pub fn focus() {
    hammerspoon::focus_alacritty()
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
    tmux(["list-windows", "-F", "#I #W"].iter())
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

fn tmux(args: Iter<&str>) -> String {
    // TODO: once
    // E.g. TMUX=/private/tmp/tmux-501/default,89323,0
    let socket_path = std::env::var("TMUX")
        .unwrap_or_else(|_| panic!("TMUX env var is not set"))
        .split(",")
        .nth(0)
        .unwrap()
        .to_string();
    let output = Command::new("tmux")
        .args(["-S", &socket_path])
        .args(args)
        .output()
        .expect("Failed to execute command");
    let stdout = str::from_utf8(&output.stdout).unwrap().to_string();
    assert!(output.stderr.is_empty());
    stdout
}
