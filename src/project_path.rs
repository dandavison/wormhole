use std::path::PathBuf;

use crate::{project::Project, tmux, vscode};

pub struct ProjectPath {
    pub project: Project,
    pub relative_path: PathBuf,
    pub line: Option<usize>,
}

impl ProjectPath {
    pub fn open(&self) -> Result<bool, String> {
        tmux::open(&self.project)?;
        vscode::open(self)?;
        Ok(true)
    }

    pub fn from_absolute_path(path: PathBuf) -> Option<Self> {
        if let Some(project) = Project::by_path(&path) {
            Some(ProjectPath {
                relative_path: path.strip_prefix(&project.path).unwrap().into(),
                project,
                line: None,
            })
        } else {
            eprintln!(
                "Path doesn't correspond to a known project: {}",
                path.to_string_lossy()
            );
            None
        }
    }

    pub fn absolute_path(&self) -> PathBuf {
        self.project.path.join(&self.relative_path)
    }
}
