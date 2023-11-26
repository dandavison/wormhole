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

pub fn read() {
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

pub fn write() -> Result<(), std::io::Error> {
    fs::write(
        projects_file(),
        projects().values().map(|p| p.format()).join("\n"),
    )
}

pub fn list_names() -> Vec<String> {
    let mut names: VecDeque<_> = projects().keys().cloned().collect();
    names.rotate_left(1);
    names.into()
}

pub fn add(path: &str, names: Vec<String>) {
    let path = PathBuf::from(path.to_string());
    let name = if !names.is_empty() {
        names[0].clone()
    } else {
        path.file_name().unwrap().to_str().unwrap().to_string()
    };

    projects().insert(
        name.clone(),
        Project {
            name,
            path,
            aliases: names,
        },
    );
    thread::spawn(write);
}

pub fn remove(name: &str) {
    projects().remove(name);
    thread::spawn(write);
}

pub fn previous() -> Option<Project> {
    projects().values().nth(1).cloned()
}
