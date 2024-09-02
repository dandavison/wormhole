use core::str;
use std::process::Command;
use std::thread;

use crate::project::Project;
use crate::terminal::write_wormhole_env_vars;
use crate::util::{get_stdout, panic};

struct Window {
    id: String,
    name: String,
}

pub fn list_window_names() -> Vec<String> {
    list_windows().into_iter().map(|w| w.name).collect()
}

pub fn exists(project: &Project) -> bool {
    get_window(&project.name).is_some()
}

pub fn open(project: &Project) -> Result<(), String> {
    println!("tmux::open({project:?})");
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
