use std::path::PathBuf;

use crate::{
    project::{get_project_by_path, Project},
    tmux, vscode,
};

pub struct ProjectPath {
    pub project: &'static Project,
    pub relative_path: PathBuf,
    pub line: Option<usize>,
}

impl ProjectPath {
    pub fn open(&self) -> Result<bool, String> {
        tmux::open(self.project)?;
        vscode::open(self)?;
        Ok(true)
    }

    pub fn from_absolute_path(path: PathBuf) -> Option<Self> {
        if let Some(project) = get_project_by_path(&path) {
            Some(ProjectPath {
                project,
                relative_path: path.strip_prefix(&project.path).unwrap().into(),
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
