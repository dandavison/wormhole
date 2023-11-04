use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::{fs, thread};

use indexmap::IndexMap;
use itertools::Itertools;
use lazy_static::lazy_static;

use crate::config;
use crate::project::Project;
use crate::util::expand_user;

lazy_static! {
    static ref PROJECTS: Mutex<IndexMap<String, Project>> = Mutex::new(IndexMap::new());
}

pub fn projects() -> MutexGuard<'static, IndexMap<String, Project>> {
    PROJECTS.lock().unwrap()
}

pub fn read_projects() {
    projects().extend(
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

pub fn write_projects() -> Result<(), std::io::Error> {
    fs::write(
        projects_file(),
        projects().values().map(|p| p.format()).join("\n"),
    )
}

pub fn list_project_names() -> Vec<String> {
    let mut names: VecDeque<_> = projects().keys().cloned().collect();
    names.rotate_left(1);
    names.into()
}

pub fn add_project(path: &str) {
    let project = Project::from_directory_path(PathBuf::from(path.to_string()));
    projects().insert(project.name.clone(), project);
    thread::spawn(write_projects);
}

pub fn remove_project(name: &str) {
    projects().remove(name);
    thread::spawn(write_projects);
}

pub fn previous_project() -> Option<Project> {
    projects().values().nth(1).cloned()
}
