use std::path::{Path, PathBuf};
use std::thread;

use crate::handlers::Destination;
use crate::util::warn;
use crate::{project::Project, tmux, vscode};

#[derive(Clone)]
pub struct ProjectPath {
    pub project: Project,
    pub relative_path: PathBuf,
    pub line: Option<usize>,
}

impl ProjectPath {
    pub fn open(&self, land_in: Option<Destination>) -> Result<bool, String> {
        let project = self.project.clone();
        let tmux_thread = thread::spawn(move || {
            tmux::open(&project).unwrap_or_else(|err| {
                warn(&format!("Error opening {} in tmux: {}", &project.name, err))
            })
        });
        let project_path = self.clone();
        let vscode_thread = thread::spawn(move || {
            vscode::open(&project_path).unwrap_or_else(|err| {
                warn(&format!(
                    "Error opening {} in vscode: {}",
                    project_path.relative_path.to_str().unwrap(),
                    err
                ))
            });
        });
        tmux_thread.join().unwrap();
        vscode_thread.join().unwrap();
        // We always focus the window for VSCode workspace, so by default, we will land in VSCode.
        if matches!(land_in, Some(Destination::Tmux)) {
            tmux::focus()
        }
        self.project.move_to_front();
        Ok(true)
    }

    pub fn from_absolute_path(path: &Path) -> Option<Self> {
        if let Some(project) = Project::by_path(&path) {
            Some(ProjectPath {
                relative_path: path.strip_prefix(&project.path).unwrap().into(),
                project,
                line: None,
            })
        } else {
            warn(&format!(
                "Path doesn't correspond to a known project: {}",
                path.to_string_lossy()
            ));
            None
        }
    }

    pub fn absolute_path(&self) -> PathBuf {
        self.project.path.join(&self.relative_path)
    }
}
