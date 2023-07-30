use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const PROJECTS_FILE: &'static str = "/Users/dan/.config/wormhole/wormhole-projects.yaml";

pub static PROJECTS: OnceLock<HashMap<String, Project>> = OnceLock::new();

pub struct Project {
    pub name: String,
    pub path: PathBuf,
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
    fs::read_to_string(PROJECTS_FILE)
        .unwrap_or_else(|_| panic!("Couldn't read projects file: {}", PROJECTS_FILE))
        .lines()
        .map(|path| PathBuf::from(path.replace("~", &home_dir.to_string_lossy())))
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .map(|name| (name.clone(), Project { name, path }))
        })
        .collect()
}
