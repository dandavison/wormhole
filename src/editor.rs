use std::fs;
use std::path::Path;

use crate::hammerspoon;
use crate::project::Project;
use crate::{project_path::ProjectPath, util::execute_command};

#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
pub enum Editor {
    None,
    Cursor,
    Emacs,
    IntelliJ,
    PyCharm,
    PyCharmCE,
    VSCode,
    VSCodeInsiders,
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
            None => "",
            Cursor => "Cursor",
            Emacs => "Emacs",
            VSCodeInsiders => "Code - Insiders",
            VSCode => "Code",
            PyCharm => "PyCharm",
            PyCharmCE => "PyCharm",
            IntelliJ => "IntelliJ",
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, None)
    }

    pub fn cli_executable_name(&self) -> &'static str {
        match self {
            None => "",
            Cursor => "cursor",
            Emacs => "emacsclient",
            VSCodeInsiders => "code-insiders",
            VSCode => "code",
            PyCharm => "pycharm",
            PyCharmCE => "pycharm",
            IntelliJ => "idea",
        }
    }

    fn open_directory_uri(&self, absolute_path: &Path) -> Option<String> {
        let path = absolute_path.to_str().unwrap();
        match self {
            None => Option::None,
            Cursor => Some(format!("cursor://file/{path}")),
            Emacs => Option::None,
            IntelliJ => Some(format!("idea://open?file={path}")),
            PyCharm => Some(format!("pycharm://open?file={path}")),
            PyCharmCE => Some(format!("pycharm://open?file={path}")),
            VSCode => Some(format!("vscode://file/{path}")),
            VSCodeInsiders => Some(format!("vscode-insiders://file/{path}")),
        }
    }

    fn open_file_uri(&self, absolute_path: &Path, line: Option<usize>) -> Option<String> {
        let path = absolute_path.to_str().unwrap();
        let line = line.unwrap_or(1);
        match self {
            None => Option::None,
            Cursor => Some(format!("cursor://file/{path}:{line}")),
            Emacs => Option::None,
            IntelliJ => Some(format!("idea://open?file={path}&line={line}")),
            PyCharm => Some(format!("pycharm://open?file={path}&line={line}")),
            PyCharmCE => Some(format!("pycharm://open?file={path}&line={line}")),
            VSCode => Some(format!("vscode://file/{path}:{line}")),
            VSCodeInsiders => Some(format!("vscode-insiders://file/{path}:{line}")),
        }
    }

    pub fn close(&self, project: &Project) {
        if self.is_none() {
            return;
        }
        let store_key = project.store_key().to_string();
        hammerspoon::close_window(self.application_name(), &store_key);
    }

    pub fn focus(&self) {
        if self.is_none() {
            return;
        }
        hammerspoon::launch_or_focus(self.application_name())
    }
}

fn workspace_file_path(project: &Project) -> std::path::PathBuf {
    let store_key = project.store_key().to_string();
    let filename = format!("{}.code-workspace", store_key);
    let gitdir = crate::git::git_common_dir(&project.repo_path);
    gitdir.join("wormhole/workspaces").join(filename)
}

fn ensure_workspace_file(project: &Project) -> std::path::PathBuf {
    let workspace_path = workspace_file_path(project);
    if !workspace_path.exists() {
        let project_dir = project.working_dir();
        let content = format!(
            r#"{{"folders": [{{"path": "{}"}}]}}"#,
            project_dir.display()
        );
        if let Some(parent) = workspace_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&workspace_path, content);
    }
    workspace_path
}

pub fn open_workspace(project: &Project) {
    ps!("open_workspace({project:?})");
    let editor = project.editor();
    if editor.is_none() {
        return;
    }
    let project_dir = project.root().absolute_path();
    match editor {
        None => {}
        Cursor | VSCode | VSCodeInsiders => {
            let workspace_path = ensure_workspace_file(project);
            execute_command(
                editor.cli_executable_name(),
                ["--new-window", workspace_path.to_str().unwrap()],
                project_dir,
            );
        }
        Emacs => {
            execute_command("emacsclient", ["-n", "."], project_dir);
        }
        IntelliJ | PyCharm | PyCharmCE => {
            execute_command(
                "bash",
                [
                    "-c",
                    &format!("{} . >& /dev/null &", editor.cli_executable_name()),
                ],
                project_dir,
            );
        }
    }
}

pub fn open_path(path: &ProjectPath) -> Result<(), String> {
    /*
       - We do two calls: one to open the workspace (i.e. analogous to `code .`)
         and one to open the path.

       - Opening a VSCode/Cursor workspace is much faster via the URI (e.g.
         vscode://file/my/project/root) than via `code .` with cwd set to the
         directory.

       - However, if the vscode window does not already exist, then
         opening via URI hijacks an existing window.

       - `open --new` with a URI doesn't actually open anything
    */
    if crate::util::debug() {
        ps!("Editor::open_path(path={path:?})");
    }
    let editor = path.project.editor();
    if editor.is_none() {
        return Ok(());
    }

    let line = path
        .relative_path
        .as_ref()
        .and_then(|(_, line)| line.to_owned());
    let root = path.project.root();
    let root_abspath = root.absolute_path();

    if *editor == Emacs {
        execute_command("emacsclient", ["-n", "."], &root_abspath);
        return Ok(());
    }

    // Open workspace file (fast via URI, sets correct window title)
    let workspace_path = ensure_workspace_file(&path.project);
    if let Some(uri) = editor.open_directory_uri(&workspace_path) {
        execute_command("open", ["-g", uri.as_str()], &root_abspath);
    }

    let file_line_uri = if path.absolute_path().is_dir() {
        Option::None
    } else {
        editor.open_file_uri(&path.absolute_path(), line)
    };
    if let Some(file_line_uri) = file_line_uri {
        execute_command("open", [file_line_uri.as_str()], &root_abspath);
    }
    Ok(())
}
