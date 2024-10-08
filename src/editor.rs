use std::path::Path;

use crate::project::Project;
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

/*
   VSCode workspace switching
   --------------------------

   Using the URL is much faster than the `code` executable, but using the
   URL with a file path cause the file to open in whatever vscode
   workspace is considered active. Since it's a URL, there's no obvious
   way to "set the PWD" for the opening process. However, issuing `open`
   twice in succession seems to work well (first for the project dir,
   then for the file+line). E.g.

   open 'cursor://file//Users/dan/src/delta'
   open 'cursor://file//Users/dan/src/delta/src/main.rs:7'

   However, this only works if the workspace is open already; otherwise
   it kills the current workspace and starts the new one. Therefore in
   ProjectPath::open we open the editor workspace (via the `code`
   executable) if the project is not open already.
*/

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

    pub fn cli_executable_name(&self) -> &'static str {
        match self {
            Cursor => "cursor",
            VSCodeInsiders => "code-insiders",
            VSCode => "code",
            PyCharm => "pycharm",
            PyCharmCE => "pycharm",
            IntelliJ => "idea",
        }
    }

    fn open_directory_uri(&self, absolute_path: &Path) -> String {
        let path = absolute_path.to_str().unwrap();
        match self {
            Cursor => format!("cursor://file/{path}"),
            IntelliJ => format!("idea://open?file={path}"),
            PyCharm => format!("pycharm://open?file={path}"),
            PyCharmCE => format!("pycharm://open?file={path}"),
            VSCode => format!("vscode://file/{path}"),
            VSCodeInsiders => format!("vscode-insiders://file/{path}"),
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

pub fn open_workspace(project: &Project) {
    ps!("open_workspace({project:?})");
    execute_command(
        config::EDITOR.cli_executable_name(),
        ["."],
        project.root().absolute_path().to_str().unwrap(),
    );
}

pub fn open_path_via_uri(path: &ProjectPath, window_action: WindowAction) -> Result<(), String> {
    let line = path
        .relative_path
        .as_ref()
        .and_then(|(_, line)| line.to_owned());
    let root = path.project.root();
    let root_abspath = root.absolute_path();
    let dir_uri = config::EDITOR.open_directory_uri(&root_abspath);
    let file_line_uri = if path.absolute_path().is_dir() {
        None
    } else {
        Some(config::EDITOR.open_file_uri(&path.absolute_path(), line))
    };
    ps!("Editor::open_path_via_uri(...)\n  path={path:?}\n  root={root:?}\n  file_line_uri={file_line_uri:?}");
    match window_action {
        WindowAction::Raise => {
            execute_command("open", [dir_uri.as_str()], &root_abspath);
            if let Some(file_line_uri) = file_line_uri {
                execute_command("open", [file_line_uri.as_str()], &root_abspath);
            }
        }
        WindowAction::Focus => {
            execute_command("open", ["-g", dir_uri.as_str()], &root_abspath);
            if let Some(file_line_uri) = file_line_uri {
                execute_command("open", ["-g", file_line_uri.as_str()], &root_abspath);
            }
        }
    }
    Ok(())
}
