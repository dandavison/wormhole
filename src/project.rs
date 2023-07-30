use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::config;
use crate::project_path::ProjectPath;

pub static PROJECTS: OnceLock<HashMap<String, Project>> = OnceLock::new();

pub struct Project {
    pub name: String,
    pub path: PathBuf,
}

impl Project {
    pub fn open(&'static self) -> Result<bool, String> {
        self.root().open()?;
        Ok(true)
    }

    fn root(&'static self) -> ProjectPath {
        ProjectPath {
            project: self,
            relative_path: "".into(),
            line: None,
        }
    }
}

pub fn get_project_by_path(query_path: &Path) -> Option<&'static Project> {
    for (_, project) in PROJECTS.get().unwrap().iter() {
        if query_path.starts_with(&project.path) {
            return Some(project);
        }
    }
    None
}

pub fn get_project_by_name(name: &str) -> Option<&'static Project> {
    PROJECTS.get().unwrap().get(name)
}

pub fn get_project_by_repo_name(repo_name: &str) -> Option<&'static Project> {
    get_project_by_name(repo_name)
}

pub fn read_projects() -> HashMap<String, Project> {
    let home_dir = dirs::home_dir().unwrap_or_else(|| panic!("Cannot determine home directory"));
    let expand_user = |p: &str| p.replace("~", &home_dir.to_string_lossy());
    let projects_file = expand_user(config::PROJECTS_FILE);
    fs::read_to_string(projects_file)
        .unwrap_or_else(|_| panic!("Couldn't read projects file: {}", config::PROJECTS_FILE))
        .lines()
        .map(|path| PathBuf::from(expand_user(path)))
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .map(|name| (name.clone(), Project { name, path }))
        })
        .collect()
}
