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

    pub fn editor(&self) -> Editor {
        if false && self.name.to_lowercase().contains("java") {
            Editor::IntelliJ
        } else if self.name == "mathematics" {
            Editor::Emacs
        } else {
            config::EDITOR
        }
    }
}
