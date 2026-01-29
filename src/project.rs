use crate::config;
use crate::editor::Editor;
use crate::git;
use crate::project_path::ProjectPath;
use crate::wormhole::Application;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Project {
    pub repo_name: String,
    pub repo_path: PathBuf,
    #[allow(unused)]
    pub aliases: Vec<String>,
    pub kv: HashMap<String, String>,
    pub last_application: Option<Application>,
    pub branch: Option<String>,
    pub github_pr: Option<u64>,
    pub github_repo: Option<String>,
}

impl Project {
    pub fn is_task(&self) -> bool {
        self.branch.is_some()
    }

    pub fn store_key(&self) -> String {
        match &self.branch {
            Some(branch) => format!("{}:{}", self.repo_name, branch),
            None => self.repo_name.clone(),
        }
    }

    pub fn worktree_path(&self) -> Option<PathBuf> {
        self.branch
            .as_ref()
            .map(|branch| git::worktree_base_path(&self.repo_path).join(branch))
    }

    pub fn working_dir(&self) -> PathBuf {
        self.worktree_path()
            .unwrap_or_else(|| self.repo_path.clone())
    }

    pub fn is_open(&self) -> bool {
        config::TERMINAL.exists(self)
    }

    pub fn as_project_path(&self) -> ProjectPath {
        ProjectPath {
            project: (*self).clone(),
            relative_path: None,
        }
    }

    pub fn root(&self) -> ProjectPath {
        ProjectPath {
            project: (*self).clone(),
            relative_path: Some(("".into(), None)),
        }
    }

    pub fn editor(&self) -> &'static Editor {
        if self.repo_name == "mathematics" {
            &Editor::Emacs
        } else {
            config::editor()
        }
    }
}
