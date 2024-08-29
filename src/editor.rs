use itertools::Itertools;
use std::ffi::OsStr;
use std::fmt::Display;
use std::path::Path;

use crate::{config, project_path::ProjectPath, util::execute_command, wormhole::WindowAction};

#[allow(dead_code)]
#[derive(Debug)]
pub enum Editor {
    Cursor,
    IntelliJ,
    VSCode,
    VSCodeInsiders,
    PyCharm,
    PyCharmCE,
}
use crate::ps;
use Editor::*;

impl Editor {
    pub fn application_name(&self) -> &'static str {
        match self {
            Cursor => "Cursor",
            VSCodeInsiders => "Code - Insiders",
            VSCode => "Code",
            PyCharm => "PyCharm",
            PyCharmCE => "PyCharm",
            IntelliJ => "IntelliJ",
        }
    }
    pub fn macos_application_bundle_name(&self) -> &'static str {
        match self {
            Cursor => "Cursor",
            VSCodeInsiders => "Visual Studio Code - Insiders",
            VSCode => "Visual Studio Code",
            PyCharm => "PyCharm",
            PyCharmCE => "PyCharm CE",
            IntelliJ => "IntelliJ IDEA",
        }
    }

    fn open_file_uri(&self, absolute_path: &Path, line: Option<usize>) -> String {
        let path = absolute_path.to_str().unwrap();
        let line = line.unwrap_or(1);
        match self {
            Cursor => format!("cursor://file/{path}:{line}"),
            IntelliJ => format!("idea://open?file={path}&line={line}"),
            PyCharm => format!("pycharm://open?file={path}&line={line}"),
            PyCharmCE => format!("pycharm://open?file={path}&line={line}"),
            VSCode => format!("vscode://file/{path}:{line}"),
            VSCodeInsiders => format!("vscode-insiders://file/{path}:{line}"),
        }
    }
}

#[allow(dead_code)]
pub fn open_path(path: &ProjectPath, window_action: WindowAction) -> Result<(), String> {
    ps!("Editor::open({path:?})");
    let project_path = path.absolute_path();
    match window_action {
        WindowAction::Raise => {
            execute_command_with_current_dir(
                config::EDITOR.application_name(),
                // HACK: VSCode-specific
                ["-g", project_path.to_str().unwrap()],
                &path.project.path,
            );
        }
        WindowAction::Focus => {
            execute_command_with_current_dir(
                "open",
                [
                    "-g",
                    "-a",
                    config::EDITOR.macos_application_bundle_name(),
                    project_path.to_str().unwrap(),
                ],
                &path.project.path,
            );
        }
    }
    Ok(())
}

pub fn open_path_via_uri(path: &ProjectPath, window_action: WindowAction) -> Result<(), String> {
    ps!("Editor::open_path_via_uri({path:?})");
    let line = path
        .relative_path
        .as_ref()
        .and_then(|(_, line)| line.to_owned());
    let uri = config::EDITOR.open_file_uri(&path.absolute_path(), line);
    match window_action {
        WindowAction::Raise => {
            execute_command_with_current_dir("open", [uri.as_str()], &path.project.path);
        }
        WindowAction::Focus => {
            execute_command_with_current_dir("open", ["-g", uri.as_str()], &path.project.path);
        }
    }
    Ok(())
}

// TODO: why does it kill and restart a workspace when using execute_command directly.,
// and when doing things like
// env -i sh -c "cd /Users/dan/src/wormhole && open 'cursor://file//Users/dan/src/wormhole/:1'"
// ?
pub fn execute_command_with_current_dir<I, S, P>(program: S, args: I, dir: P) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    S: Copy,
    S: Display,
    P: AsRef<Path>,
{
    let dir: &str = dir.as_ref().to_str().unwrap_or(".");
    execute_command(
        "sh",
        [
            "-c",
            &format!(
                "PWD={} cd {} && {program} {}",
                dir,
                dir,
                args.into_iter().join(" ")
            ),
        ],
        dir,
    )
}
