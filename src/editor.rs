use std::{path::Path, process::Command};

use crate::{
    config, hammerspoon, project::Project, project_path::ProjectPath, util::warn, WindowAction,
};

#[allow(dead_code)]
pub enum Editor {
    IntelliJ,
    VSCode,
    VSCodeInsiders,
    PyCharm,
}
use Editor::*;

impl Editor {
    fn open_file_uri(&self, absolute_path: &Path, line: Option<usize>) -> String {
        let path = absolute_path.to_str().unwrap();
        let line = line.unwrap_or(1);
        match self {
            IntelliJ => format!("idea://open?file={path}&line={line}"),
            PyCharm => format!("pycharm://open?file={path}&line={line}"),
            VSCode => format!("vscode://file/{path}:{line}"),
            VSCodeInsiders => format!("vscode-insiders://file/{path}:{line}"),
        }
    }

    pub fn application_name(&self) -> &'static str {
        match self {
            IntelliJ => "IntelliJ",
            PyCharm => "PyCharm",
            VSCode => "Code",
            VSCodeInsiders => "Code - Insiders",
        }
    }
}

pub fn open_project(project: &Project, window_action: WindowAction) -> Result<(), String> {
    hammerspoon::select_editor_workspace(config::EDITOR, project, window_action)
}

pub fn open_path(path: &ProjectPath, window_action: WindowAction) -> Result<(), String> {
    open_project(&path.project, window_action)?;
    if path.relative_path.is_some() {
        open_editor_application_at_path(path)?
    }
    Ok(())
}

fn open_editor_application_at_path(path: &ProjectPath) -> Result<(), String> {
    let line = path
        .relative_path
        .as_ref()
        .and_then(|(_, line)| line.to_owned());
    let uri = config::EDITOR.open_file_uri(&path.absolute_path(), line);

    warn(&format!("open_editor_application_at_path({uri})"));
    if let Ok(_) = Command::new("open").arg(&uri).output() {
        Ok(())
    } else {
        Err(format!("Failed to open URI: {}", uri))
    }
}
