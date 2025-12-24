use crate::config;
use crate::editor::Editor;
use crate::project_path::ProjectPath;
use crate::util::{expand_user, panic};
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

    pub fn parse(line: &str) -> Self {
        let parts: Vec<&str> = line.split("->").collect();
        let path = PathBuf::from(expand_user(parts[0].trim()));
        let (name, aliases) = if parts.len() > 1 {
            let names: Vec<String> = parts[1].split(",").map(|s| s.trim().to_string()).collect();
            (names[0].clone(), names)
        } else {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| {
                    // Handle special cases where path doesn't have a file name
                    if path == PathBuf::from("/") {
                        panic("Cannot use root directory '/' as a project path");
                    } else {
                        panic(&format!(
                            "Invalid project path (no file name): {}",
                            path.display()
                        ))
                    }
                });
            (name, vec![])
        };
        Self {
            name,
            path,
            aliases,
            kv: HashMap::new(),
            last_application: None,
        }
    }

    pub fn is_terminal_only(&self) -> bool {
        self.name == "services"
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
