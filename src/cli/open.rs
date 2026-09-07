//! Open what a piece of text names, and answer whether it names anything.
//!
//! `no_create` is for text the user did not type, such as a terminal click
//! target: it suppresses everything that would bring a task into existence, so
//! that text naming nothing that already exists is an error rather than a new
//! worktree. `openable` answers the same question the click does, ahead of the
//! click, so that a terminal can underline only what a click would open.

use std::path::PathBuf;

use super::task;
use super::util::*;
use crate::target::{self, Target};

pub(super) fn run(
    client: &Client,
    target: &str,
    land_in: Option<String>,
    home_project: Option<String>,
    dry_run: bool,
    no_create: bool,
) -> Result<(), String> {
    if task::is_create_ref(target) && !no_create {
        return task::task_create(client, target, home_project, land_in, dry_run);
    }
    let parsed = target::parse(target, &directories(client))
        .ok_or_else(|| format!("No wormhole target in {:?}", target))?;
    match parsed {
        Target::Conversation(path) => {
            client.post(&format!("/conversations/resume/{}", path.display()))?;
        }
        Target::File { path, line } => {
            let query = build_query(&Some("editor".to_string()), &line);
            client.get(&format!("/file/{}{}", path.display(), query))?;
        }
        Target::Directory(path) => {
            let query = build_switch_query(&land_in, &None, &None, &None);
            client.get(&format!("/project/switch/{}{}", path.display(), query))?;
        }
        Target::Project(name) => {
            if no_create && !is_known_project(client, &name)? {
                return Err(format!("Unknown project '{}'", name));
            }
            let query = build_switch_query(&land_in, &None, &None, &None);
            client.get(&format!("/project/switch/{}{}", name, query))?;
        }
    }
    Ok(())
}

/// Print those of `targets` which `open --no-create` would open, one per line.
pub(super) fn openable(client: &Client, targets: &[String]) -> Result<(), String> {
    let directories = directories(client);
    for text in targets {
        let opens = match target::parse(text, &directories) {
            Some(Target::Project(name)) => is_known_project(client, &name)?,
            Some(_) => true,
            None => false,
        };
        if opens {
            println!("{}", text);
        }
    }
    Ok(())
}

/// Directories a relative path in `target` may be relative to: this process's,
/// for text typed here, then those of the terminal panes in view, for text
/// read off the screen. Wormhole's own window knows the latter; a click arrives
/// with the terminal's directory, which is no relation to what was clicked.
fn directories(client: &Client) -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = std::env::current_dir().into_iter().collect();
    if let Ok(response) = client.get("/terminal/pane-directories") {
        directories.extend(
            response
                .lines()
                .filter(|line| !line.is_empty())
                .map(PathBuf::from),
        );
    }
    directories.dedup();
    directories
}

fn is_known_project(client: &Client, name: &str) -> Result<bool, String> {
    let response = client.get("/project/worktrees")?;
    let projects: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    let projects = projects.as_array().ok_or("Expected an array of projects")?;
    Ok(projects.iter().any(|project| {
        [
            project.get("project_key"),
            project.get("repo").filter(|_| !name.contains(':')),
        ]
        .iter()
        .flatten()
        .any(|value| value.as_str() == Some(name))
    }))
}
