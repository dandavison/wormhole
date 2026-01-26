use crate::project::Project;
use crate::util::execute_command;
use crate::wormhole::Application;
use crate::{config, ps};
use lazy_static::lazy_static;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

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
    pub fn all(&self) -> &VecDeque<Project> {
        &self.0
    }

    pub fn all_mut(&mut self) -> &mut VecDeque<Project> {
        &mut self.0
    }

    pub fn names(&self) -> Vec<String> {
        self.0.iter().map(|p| p.name.clone()).collect()
    }

    pub fn previous(&self) -> Option<Project> {
        self.0.get(1).cloned()
    }

    pub fn current(&self) -> Option<Project> {
        self.0.get(0).cloned()
    }

    pub fn next(&self) -> Option<Project> {
        self.0.back().cloned()
    }

    pub fn apply(&mut self, mutation: Mutation, name: &str) {
        match mutation {
            Mutation::Insert => {
                self.move_to_back(name);
                self.0.rotate_right(1);
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
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        // Never add home directory as a project
        if Some(path.as_path()) == dirs::home_dir().as_deref() {
            return;
        }
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
                kv: std::collections::HashMap::new(),
                last_application: None,
                home_project: None,
            });
        }
    }

    pub fn add_project(&mut self, project: Project) {
        if Some(project.path.as_path()) == dirs::home_dir().as_deref() {
            return;
        }
        if !self.contains(&project.name) {
            ps!("projects::add_project");
            self.0.push_front(project);
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.index_by_name(name).map(|i| {
            self.0.remove(i);
        });
    }

    pub fn move_to_back(&mut self, name: &str) {
        self.index_by_name(&name).map(|i| {
            self.0.remove(i).map(|p| {
                self.0.push_back(p);
            });
        });
    }

    pub fn set_last_application(&mut self, name: &str, application: Application) {
        if let Some(i) = self.index_by_name(name) {
            self.0[i].last_application = Some(application);
        }
    }

    fn _insert_right(&mut self, p: Project) {
        let index = if self.0.is_empty() { 0 } else { 1 };
        self.0.insert(index, p);
    }

    /// Find project whose path is a prefix of query_path (for file lookups)
    pub fn by_path(&self, query_path: &Path) -> Option<Project> {
        let query_path =
            std::fs::canonicalize(query_path).unwrap_or_else(|_| query_path.to_path_buf());
        self.0
            .iter()
            .filter(|p| query_path.starts_with(&p.path))
            .max_by_key(|p| p.path.as_os_str().len())
            .cloned()
    }

    /// Find project at exactly this path (for project switching)
    pub fn by_exact_path(&self, path: &Path) -> Option<Project> {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.0.iter().find(|p| p.path == path).cloned()
    }

    pub fn by_name(&self, name: &str) -> Option<Project> {
        self.0.iter().find(|p| p.name == name).cloned()
    }

    pub fn resolve(&self, name: &str) -> Option<Project> {
        crate::task::get_task(name).or_else(|| self.by_name(name))
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
        let previous = self.previous().map(|p| p.name).unwrap_or("none".into());
        let current = self.current().map(|p| p.name).unwrap_or("none".into());
        let next = self.next().map(|p| p.name).unwrap_or("none".into());
        let len = self.0.len();

        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            println!("{}", execute_command("vscode-summary", [], "/tmp"));
            println!("");
            ps!("..., {}, {}*, {}, ... ({})", previous, current, next, len,);
        });
    }
}

pub fn load() {
    let mut projects = lock();

    // Build a reverse map from canonical path to disambiguated name
    let available = config::available_projects();
    let path_to_name: std::collections::HashMap<PathBuf, String> = available
        .into_iter()
        .filter_map(|(name, path)| {
            std::fs::canonicalize(&path)
                .ok()
                .map(|canonical| (canonical, name))
        })
        .collect();

    for dir in config::TERMINAL.project_directories() {
        let path = PathBuf::from(&dir);
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());

        // Use the disambiguated name if available, otherwise derive from directory
        let name = path_to_name.get(&canonical).cloned().unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        // Skip if a project with this name already exists
        if !projects.contains(&name) {
            projects.0.push_back(Project {
                name,
                path: canonical,
                aliases: vec![],
                kv: std::collections::HashMap::new(),
                last_application: None,
                home_project: None,
            });
        }
    }

    crate::kv::load_kv_data(&mut projects);
    if crate::util::debug() {
        projects.print();
    }
}
