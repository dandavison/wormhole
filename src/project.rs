use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use indexmap::IndexMap;
use lazy_static::lazy_static;

use crate::config;
use crate::project_path::ProjectPath;

lazy_static! {
    static ref PROJECTS: Mutex<IndexMap<String, Project>> = Mutex::new(IndexMap::new());
}

#[derive(Clone)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
}

impl Project {
    pub fn open(&self) -> Result<bool, String> {
        self.root().open()?;
        Ok(true)
    }

    fn root(&self) -> ProjectPath {
        ProjectPath {
            project: (*self).clone(),
            relative_path: "".into(),
            line: None,
        }
    }
    pub fn by_path(query_path: &Path) -> Option<Self> {
        for project in PROJECTS.lock().unwrap().values() {
            if query_path.starts_with(&project.path) {
                return Some(project.clone());
            }
        }
        None
    }

    pub fn by_name(name: &str) -> Option<Self> {
        PROJECTS.lock().unwrap().get(name).cloned()
    }

    pub fn by_repo_name(repo_name: &str) -> Option<Self> {
        Self::by_name(repo_name)
    }
}

pub fn read_projects() {
    let home_dir = dirs::home_dir().unwrap_or_else(|| panic!("Cannot determine home directory"));
    let expand_user = |p: &str| p.replace("~", &home_dir.to_string_lossy());
    let projects_file = expand_user(config::PROJECTS_FILE);
    PROJECTS.lock().unwrap().extend(
        fs::read_to_string(projects_file)
            .unwrap_or_else(|_| panic!("Couldn't read projects file: {}", config::PROJECTS_FILE))
            .lines()
            .map(|path| PathBuf::from(expand_user(path)))
            .filter_map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .map(|name| (name.clone(), Project { name, path }))
            }),
    )
}

pub fn list_project_names() -> Vec<String> {
    PROJECTS.lock().unwrap().keys().cloned().collect()
}
