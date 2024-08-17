use std::str;
use std::{process::Command, slice::Iter};

use crate::project::Project;
use crate::util::info;

#[allow(dead_code)]
struct Pane {
    id: String,
    tab_id: String,
    win_id: String,
    workspace: String,
    title: String,
    cwd: String,
}

pub fn open(project: &Project) -> Result<(), String> {
    info(&format!("wezterm::open({project:?})"));
    let mut pane = get_pane(&project.name);
    if pane.is_none() {
        wezterm(
            [
                "cli",
                "spawn",
                "--new-window",
                "--workspace",
                &project.name,
                "--cwd",
                &project.path.to_str().unwrap(),
            ]
            .iter(),
        );
        pane = get_pane(&project.name)
    }
    if let Some(pane) = pane {
        wezterm(["cli", "activate-pane", "--pane-id", &pane.id].iter());
        Ok(())
    } else {
        Err(format!(
            "Failed to spawn wezterm pane for project {}",
            &project.name
        ))
    }
}

fn get_pane(name: &str) -> Option<Pane> {
    for p in list_panes() {
        if p.workspace == name {
            return Some(p);
        }
    }
    None
}

fn list_panes() -> Vec<Pane> {
    wezterm(["cli", "list"].iter())
        .split_terminator("\n")
        .map(|line| {
            let mut fields = line.split(" ");
            Pane {
                win_id: fields.next().unwrap().to_string(),
                tab_id: fields.next().unwrap().to_string(),
                id: fields.next().unwrap().to_string(),
                workspace: fields.next().unwrap().to_string(),
                title: fields.next().unwrap().to_string(),
                cwd: fields.next().unwrap().to_string(),
            }
        })
        .collect()
}

pub fn wezterm(args: Iter<&str>) -> String {
    let output = Command::new("wezterm")
        .args(args)
        .output()
        .expect("stdout should be available");
    let stdout = str::from_utf8(&output.stdout).unwrap().to_string();
    assert!(output.stderr.is_empty());
    stdout
}
