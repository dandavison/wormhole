use crate::config;
use crate::editor::Editor;
use crate::project_path::ProjectPath;
use crate::wormhole::Application;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    #[allow(unused)]
    pub aliases: Vec<String>,
    pub kv: HashMap<String, String>,
    pub last_application: Option<Application>,
    pub home_project: Option<String>,
    pub github_pr: Option<u64>,
    pub github_repo: Option<String>,
}

impl Project {
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
        if self.name == "mathematics" {
            &Editor::Emacs
        } else {
            config::editor()
        }
    }
}
