use crate::project_path::ProjectPath;
use crate::projects::{self, projects};
use crate::util::{contract_user, expand_user};
use std::path::{Path, PathBuf};
use std::thread;

#[derive(Clone, Debug)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
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
        projects().get(name).cloned()
    }

    pub fn by_repo_name(repo_name: &str) -> Option<Self> {
        Self::by_name(repo_name)
    }

    pub fn move_to_front(&self) {
        let idx = projects().get_index_of(&self.name).unwrap();
        projects().move_index(idx, 0);
        thread::spawn(projects::write);
    }

    pub fn from_directory_path(path: PathBuf) -> Self {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        Self { name, path }
    }

    pub fn parse(line: &str) -> Self {
        let parts: Vec<&str> = line.split("->").collect();
        let path = PathBuf::from(expand_user(parts[0].trim()));
        let name = if parts.len() > 1 {
            parts[1].trim().to_string()
        } else {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap()
        };
        Self { name, path }
    }

    pub fn format(&self) -> String {
        let mut s = contract_user(self.path.to_str().unwrap());
        if self.name != self.path.file_name().unwrap().to_str().unwrap() {
            s += &format!(" -> {}", self.name);
        }
        s
    }
}
