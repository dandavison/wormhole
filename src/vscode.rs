use std::process::Command;

use crate::{hammerspoon, project_path::ProjectPath, tmux};

pub fn open(path: ProjectPath) -> Result<bool, String> {
    tmux::set_project(path.project)?;
    let mut uri = format!(
        "vscode-insiders://file/{}",
        path.absolute_path().to_str().unwrap()
    );
    if let Some(line) = path.line {
        uri.push_str(&format!(":{}", line));
    }
    hammerspoon::focus_vscode_workspace(&path.project.name)?;
    eprintln!("vscode::open({uri})");
    if let Ok(_) = Command::new("open").arg(&uri).output() {
        Ok(true)
    } else {
        Err(format!("Failed to open URI: {}", uri))
    }
}
