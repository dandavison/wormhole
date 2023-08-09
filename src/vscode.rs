use std::process::Command;

use crate::{hammerspoon, project::Project, project_path::ProjectPath, util::warn, WindowAction};

pub fn open_project(project: &Project, window_action: WindowAction) -> Result<(), String> {
    hammerspoon::select_vscode_workspace(&project.name, window_action)
}

pub fn open_path(path: &ProjectPath, window_action: WindowAction) -> Result<(), String> {
    open_project(&path.project, window_action)?;
    if let Some((_, line)) = path.relative_path {
        let mut uri = format!(
            "vscode-insiders://file/{}",
            path.absolute_path().to_str().unwrap()
        );
        if let Some(line) = line {
            uri.push_str(&format!(":{}", line));
        }
        warn(&format!("vscode::open({uri})"));
        if let Ok(_) = Command::new("open").arg(&uri).output() {
            Ok(())
        } else {
            Err(format!("Failed to open URI: {}", uri))
        }
    } else {
        Ok(())
    }
}
