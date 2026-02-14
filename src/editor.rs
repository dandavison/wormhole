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

fn wormhole_workspace_path(project: &Project) -> std::path::PathBuf {
    let store_key = project.store_key().to_string();
    let filename = format!("{}.code-workspace", store_key.replace('/', "%2F"));
    let gitdir = crate::git::git_common_dir(&project.repo_path);
    gitdir.join("wormhole/workspaces").join(filename)
}

fn root_workspace_file(project: &Project) -> Option<std::path::PathBuf> {
    let root = project.working_tree();
    let entries = fs::read_dir(&root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "code-workspace") {
            return Some(path);
        }
    }
    Option::None
}

fn resolve_workspace_file(project: &Project) -> Result<std::path::PathBuf, String> {
    let root_ws = root_workspace_file(project);
    let wormhole_ws = wormhole_workspace_path(project);
    let wormhole_exists = wormhole_ws.exists();

    match (root_ws, wormhole_exists) {
        (Some(root), true) => Err(format!(
            "Workspace file conflict: both '{}' and '{}' exist",
            root.display(),
            wormhole_ws.display()
        )),
        (Some(root), false) => Ok(root),
        (Option::None, true) => Ok(wormhole_ws),
        (Option::None, false) => {
            let project_dir = project.working_tree();
            let content = format!(
                r#"{{"folders": [{{"path": "{}"}}]}}"#,
                project_dir.display()
            );
            if let Some(parent) = wormhole_ws.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&wormhole_ws, content);
            Ok(wormhole_ws)
        }
    }
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
            let workspace_path = match resolve_workspace_file(project) {
                Ok(p) => p,
                Err(e) => {
                    crate::util::error(&e);
                    return;
                }
            };
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

/// Migrate workspace files whose folder paths point to branch-level directories
/// (old worktree layout) instead of repo-level directories (new layout).
pub fn migrate_workspace_files(repo_path: &Path) -> Result<usize, String> {
    let ws_dir = crate::git::git_common_dir(repo_path).join("wormhole/workspaces");
    if !ws_dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in fs::read_dir(&ws_dir)
        .map_err(|e| format!("Failed to read {}: {}", ws_dir.display(), e))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "code-workspace") {
            continue;
        }
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let content = strip_trailing_commas(&content);
        let mut doc: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
        let mut changed = false;
        if let Some(folders) = doc.get_mut("folders").and_then(|f| f.as_array_mut()) {
            for folder in folders.iter_mut() {
                let p = folder.get("path").and_then(|p| p.as_str()).map(String::from);
                if let Some(ref p) = p {
                    if let Some(child) = sole_worktree_child(Path::new(p)) {
                        folder["path"] =
                            serde_json::Value::String(child.display().to_string());
                        changed = true;
                    }
                }
            }
        }
        if changed {
            let new_content = serde_json::to_string(&doc)
                .map_err(|e| format!("Failed to serialize: {}", e))?;
            fs::write(&path, &new_content)
                .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
            count += 1;
        }
    }
    Ok(count)
}

/// Strip trailing commas before `]` and `}` (JSONC → JSON).
fn strip_trailing_commas(s: &str) -> String {
    regex::Regex::new(r",\s*([}\]])")
        .unwrap()
        .replace_all(s, "$1")
        .into_owned()
}

/// If `dir` is not itself a worktree but contains exactly one child that is,
/// return that child's path.
fn sole_worktree_child(dir: &Path) -> Option<std::path::PathBuf> {
    if !dir.is_dir() || dir.join(".git").is_file() {
        return Option::None;
    }
    let mut children = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join(".git").is_file());
    let first = children.next()?;
    if children.next().is_some() {
        return Option::None;
    }
    Some(first)
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
    let workspace_path = resolve_workspace_file(&path.project)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrate_workspace_files() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");

        // Set up fake git repo so git_common_dir works
        let gitdir = repo.join(".git");
        let ws_dir = gitdir.join("wormhole/workspaces");
        let worktrees_dir = gitdir.join("wormhole/worktrees");
        fs::create_dir_all(&ws_dir).unwrap();

        // Simulate post-migration worktree layout: branch_dir/repo_name/.git
        let branch_dir = worktrees_dir.join("sa-status");
        let repo_worktree = branch_dir.join("myrepo");
        fs::create_dir_all(&repo_worktree).unwrap();
        fs::write(repo_worktree.join(".git"), "gitdir: fake").unwrap();

        // Write a stale workspace file pointing to the branch-level directory
        let ws_file = ws_dir.join("myrepo:sa-status.code-workspace");
        let old_content = format!(
            r#"{{"folders":[{{"path":"{}"}}]}}"#,
            branch_dir.display()
        );
        fs::write(&ws_file, &old_content).unwrap();

        let count = migrate_workspace_files(&repo).unwrap();
        assert_eq!(count, 1);

        let updated: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&ws_file).unwrap()).unwrap();
        let folder_path = updated["folders"][0]["path"].as_str().unwrap();
        assert_eq!(folder_path, repo_worktree.display().to_string());

        // Running again is a no-op
        assert_eq!(migrate_workspace_files(&repo).unwrap(), 0);
    }

    #[test]
    fn test_migrate_workspace_files_already_correct() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");

        let gitdir = repo.join(".git");
        let ws_dir = gitdir.join("wormhole/workspaces");
        let worktrees_dir = gitdir.join("wormhole/worktrees");
        fs::create_dir_all(&ws_dir).unwrap();

        let repo_worktree = worktrees_dir.join("sa-status").join("myrepo");
        fs::create_dir_all(&repo_worktree).unwrap();
        fs::write(repo_worktree.join(".git"), "gitdir: fake").unwrap();

        // Workspace already points to the correct repo-level path
        let ws_file = ws_dir.join("myrepo:sa-status.code-workspace");
        let content = format!(
            r#"{{"folders":[{{"path":"{}"}}]}}"#,
            repo_worktree.display()
        );
        fs::write(&ws_file, &content).unwrap();

        assert_eq!(migrate_workspace_files(&repo).unwrap(), 0);
    }

    #[test]
    fn test_sole_worktree_child() {
        let temp = tempfile::tempdir().unwrap();

        // Directory with one worktree child
        let parent = temp.path().join("branch");
        let child = parent.join("myrepo");
        fs::create_dir_all(&child).unwrap();
        fs::write(child.join(".git"), "gitdir: fake").unwrap();
        assert_eq!(sole_worktree_child(&parent), Some(child.clone()));

        // Directory that IS a worktree (has .git file)
        assert_eq!(sole_worktree_child(&child), Option::None);

        // Directory with multiple worktree children
        let child2 = parent.join("other");
        fs::create_dir_all(&child2).unwrap();
        fs::write(child2.join(".git"), "gitdir: fake").unwrap();
        assert_eq!(sole_worktree_child(&parent), Option::None);

        // Non-existent directory
        assert_eq!(sole_worktree_child(temp.path().join("nope").as_path()), Option::None);
    }
}
