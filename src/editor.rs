use std::{path::Path, process::Command};

use crate::{
    config, hammerspoon,
    project::Project,
    project_path::ProjectPath,
    tmux,
    util::{error, info, warn},
    wormhole::WindowAction,
};

#[allow(dead_code)]
#[derive(Debug)]
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
            VSCodeInsiders => "Code - Insiders",
            VSCode => "Code",
            PyCharm => "PyCharm",
            IntelliJ => "IntelliJ",
        }
    }
}

fn open_project(project: &Project) -> Result<(), String> {
    let executable = match config::EDITOR {
        VSCode | VSCodeInsiders => "code",
        PyCharm => "pycharm",
        IntelliJ => "idea",
    };
    tmux::tmux(
        [
            "send-keys",
            "-t",
            &project.name,
            &format!("{executable} ."),
            "Enter",
        ]
        .iter(),
    );
    Ok(())
}

fn select_project(project: &Project, window_action: &WindowAction) -> bool {
    hammerspoon::select_editor_workspace(config::EDITOR, project, window_action)
}

pub fn open_path(path: &ProjectPath, window_action: WindowAction) -> Result<(), String> {
    info(&format!("editor::open_path({path:?}, {window_action:?})"));
    if !select_project(&path.project, &window_action) {
        info("Failed to select project; trying to open workspace");
        open_project(&path.project)?;
        if !select_project(&path.project, &window_action) {
            error("Failed to find editor workspace after opening editor in project directory")
        }
    }
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

    info(&format!("open_editor_application_at_path({uri})"));
    if let Err(err) = Command::new("open").arg(&uri).spawn() {
        Err(format!("Failed to open URI: {}: {}", uri, err))
    } else {
        Ok(())
    }
}
