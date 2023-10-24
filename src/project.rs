use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::{fs, thread};

use indexmap::IndexMap;
use itertools::Itertools;
use lazy_static::lazy_static;

use crate::config;
use crate::project_path::ProjectPath;

lazy_static! {
    static ref PROJECTS: Mutex<IndexMap<String, Project>> = Mutex::new(IndexMap::new());
}

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

    pub fn move_to_front(&self) {
        let idx = PROJECTS.lock().unwrap().get_index_of(&self.name).unwrap();
        PROJECTS.lock().unwrap().move_index(idx, 0);
        thread::spawn(write_projects);
    }

    fn from_directory_path(path: PathBuf) -> Self {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        Self { name, path }
    }

    fn parse(line: &str) -> Self {
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

    fn format(&self) -> String {
        let mut s = contract_user(self.path.to_str().unwrap());
        if self.name != self.path.file_name().unwrap().to_str().unwrap() {
            s += &format!(" -> {}", self.name);
        }
        s
    }
}

pub fn read_projects() {
    PROJECTS.lock().unwrap().extend(
        fs::read_to_string(projects_file())
            .unwrap_or_else(|_| panic!("Couldn't read projects file: {}", config::PROJECTS_FILE))
            .lines()
            .map(Project::parse)
            .map(|proj| (proj.name.clone(), proj)),
    )
}

fn projects_file() -> String {
    expand_user(config::PROJECTS_FILE)
}

fn expand_user(path: &str) -> String {
    path.replacen("~", &home_dir().to_str().unwrap(), 1)
}

fn contract_user(path: &str) -> String {
    path.replacen(&home_dir().to_str().unwrap(), "~", 1)
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| panic!("Cannot determine home directory"))
}

pub fn write_projects() -> Result<(), std::io::Error> {
    fs::write(
        projects_file(),
        PROJECTS
            .lock()
            .unwrap()
            .values()
            .map(|p| p.format())
            .join("\n"),
    )
}

pub fn list_project_names() -> Vec<String> {
    let mut names: VecDeque<_> = PROJECTS.lock().unwrap().keys().cloned().collect();
    names.rotate_left(1);
    names.into()
}

pub fn add_project(path: &str) {
    let mut projects = PROJECTS.lock().unwrap();
    let project = Project::from_directory_path(PathBuf::from(path.to_string()));
    projects.insert(project.name.clone(), project);
    thread::spawn(write_projects);
}

pub fn remove_project(name: &str) {
    let mut projects = PROJECTS.lock().unwrap();
    projects.remove(name);
    thread::spawn(write_projects);
}

pub fn previous_project() -> Option<Project> {
    PROJECTS.lock().unwrap().values().nth(1).cloned()
}
