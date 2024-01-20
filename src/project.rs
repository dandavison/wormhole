use crate::project_path::ProjectPath;
use crate::projects::{self, projects};
use crate::util::{contract_user, expand_user};
use std::path::{Path, PathBuf};
use std::thread;

#[derive(Clone, Debug)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub aliases: Vec<String>,
}

impl Project {
    pub fn as_project_path(&self) -> ProjectPath {
        ProjectPath {
            project: (*self).clone(),
            relative_path: None,
        }
    }

    #[allow(dead_code)]
    pub fn root(&self) -> ProjectPath {
        ProjectPath {
            project: (*self).clone(),
            relative_path: Some(("".into(), None)),
        }
    }

    pub fn by_path(query_path: &Path) -> Option<Self> {
        for project in projects().values() {
            if query_path.starts_with(&project.path) {
                return Some(project.clone());
            }
        }
        None
    }

    pub fn by_name(name: &str) -> Option<Self> {
        let projects = projects();
        if let Some(project) = projects.get(name) {
            Some(project.clone())
        } else {
            for project in projects.values() {
                if project.aliases.iter().find(|&a| a == name).is_some() {
                    return Some(project.clone());
                }
            }
            None
        }
    }

    pub fn by_repo_name(repo_name: &str) -> Option<Self> {
        Self::by_name(repo_name)
    }

    pub fn move_to_front(&self) {
        let idx = projects().get_index_of(&self.name).unwrap();
        projects().move_index(idx, 0);
        thread::spawn(projects::write);
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
                .unwrap();
            (name, vec![])
        };
        Self {
            name,
            path,
            aliases,
        }
    }

    pub fn format(&self) -> String {
        let mut s = contract_user(self.path.to_str().unwrap());
        if !self.aliases.is_empty() {
            s += &format!(" -> {}", self.aliases.join(", "));
        }
        s
    }

    pub fn is_terminal_only(&self) -> bool {
        self.name == "services"
    }
}
