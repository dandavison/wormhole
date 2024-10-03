use crate::project::Project;
use crate::{config, ps};
use lazy_static::lazy_static;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/*
    - Projects are held in a ring.

    - The currently active project is at index 0.

    - When adding a new project, we insert it to the right of the current project,
      i.e. at index 1 (if there is a current project).

    - When jumping to a project, we remove it and insert it to the right of the
      current project.

    - When switching to the previous project, we rotate right.

    - When switching to the next project, or selecting one we just added, or are jumping to,
      we rotate left.

    - Write to disk asynchronously after every mutation.
*/

// TODO: Wormhole doesn't need to support any concurrency; perhaps serialize
// request processing and don't use a lock at all?
lazy_static! {
    static ref PROJECTS: Mutex<VecDeque<Project>> = Mutex::new(VecDeque::new());
}

pub struct Projects<'a>(MutexGuard<'a, VecDeque<Project>>);

pub fn lock<'a>() -> Projects<'a> {
    Projects(PROJECTS.lock().unwrap())
}

#[derive(Debug)]
pub enum Mutation {
    RotateLeft,
    RotateRight,
    Insert,
}

impl<'a> Projects<'a> {
    pub fn names(&self) -> Vec<String> {
        self.0.iter().map(|p| p.name.clone()).collect()
    }

    pub fn previous(&self) -> Option<Project> {
        self.0.back().cloned()
    }

    pub fn current(&self) -> Option<Project> {
        self.0.get(0).cloned()
    }

    pub fn next(&self) -> Option<Project> {
        self.0.get(1).cloned()
    }

    pub fn apply(&mut self, mutation: Mutation, name: &str) {
        match mutation {
            Mutation::Insert => {
                self.move_to_front(name);
                self.0.rotate_left(1);
            }
            Mutation::RotateLeft => self.0.rotate_left(1),
            Mutation::RotateRight => self.0.rotate_right(1),
        };
    }

    pub fn open(&self) -> Vec<Project> {
        let terminal_windows = config::TERMINAL.window_names();
        self.0
            .iter()
            .filter(|p| terminal_windows.contains(&p.name))
            .cloned()
            .collect()
    }

    pub fn add(&mut self, path: &str, names: Vec<String>) {
        let path = PathBuf::from(path.to_string());
        let name = if !names.is_empty() {
            names[0].clone()
        } else {
            path.file_name().unwrap().to_str().unwrap().to_string()
        };
        if !self.contains(&name) {
            ps!("projects::add");
            self.0.push_front(Project {
                name,
                path,
                aliases: names,
            });
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.index_by_name(name).map(|i| {
            self.0.remove(i);
        });
    }

    pub fn move_to_front(&mut self, name: &str) {
        self.index_by_name(&name).map(|i| {
            self.0.remove(i).map(|p| {
                self.0.push_front(p);
            });
        });
    }

    fn _insert_right(&mut self, p: Project) {
        let index = if self.0.is_empty() { 0 } else { 1 };
        self.0.insert(index, p);
    }

    pub fn by_path(&self, query_path: &Path) -> Option<Project> {
        self.0.iter().find_map(|p| {
            // The query path may have a `:line` suffix.
            if query_path.starts_with(&p.path) {
                Some(p.clone())
            } else {
                None
            }
        })
    }

    pub fn by_name(&self, name: &str) -> Option<Project> {
        self.0.iter().find_map(|p| {
            if p.name == name {
                Some(p.clone())
            } else {
                None
            }
        })
    }

    fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|p| p.name == name)
    }

    fn index_by_name(&self, name: &str) -> Option<usize> {
        self.0
            .iter()
            .enumerate()
            .find_map(|(i, p)| if p.name == name { Some(i) } else { None })
    }

    pub fn print(&self) {
        ps!(
            "..., {}, {}*, {}, ... ({})",
            self.previous().map(|p| p.name).unwrap_or("none".into()),
            self.current().map(|p| p.name).unwrap_or("none".into()),
            self.next().map(|p| p.name).unwrap_or("none".into()),
            self.0.len(),
        );
    }
}

pub fn load() {
    let mut projects = lock();
    projects.0.extend(
        config::TERMINAL
            .project_directories()
            .iter()
            .map(|p| Project::parse(p)),
    );
    projects.print();
}
