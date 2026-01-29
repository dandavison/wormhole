use crate::project::{Project, StoreKey};
use crate::util::execute_command;
use crate::wormhole::Application;
use crate::{config, git, ps};
use lazy_static::lazy_static;
use rayon::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

/*
    - Projects are held in a ring for navigation (previous/current/next).
    - All known projects (including tasks) are stored in a HashMap.
    - The ring contains keys that reference the HashMap.

    - The currently active project is at ring index 0.
    - When adding a new project, we insert it to the right of current (index 1).
    - When jumping to a project, we move it to the right of current.
    - When switching to previous, we rotate right.
    - When switching to next, we rotate left.
*/

struct Store {
    all: HashMap<StoreKey, Project>,
    ring: VecDeque<StoreKey>,
}

lazy_static! {
    static ref STORE: Mutex<Store> = Mutex::new(Store {
        all: HashMap::new(),
        ring: VecDeque::new(),
    });
}

pub struct Projects<'a>(MutexGuard<'a, Store>);

pub fn lock<'a>() -> Projects<'a> {
    Projects(STORE.lock().unwrap())
}

#[derive(Debug)]
pub enum Mutation {
    None,
    RotateLeft,
    RotateRight,
    Insert,
}

impl<'a> Projects<'a> {
    pub fn all(&self) -> Vec<&Project> {
        self.0
            .ring
            .iter()
            .filter_map(|k| self.0.all.get(k))
            .collect()
    }

    pub fn all_mut(&mut self) -> impl Iterator<Item = &mut Project> {
        self.0.all.values_mut()
    }

    pub fn keys(&self) -> Vec<StoreKey> {
        self.0.ring.iter().cloned().collect()
    }

    pub fn previous(&self) -> Option<Project> {
        self.0.ring.get(1).and_then(|k| self.0.all.get(k)).cloned()
    }

    pub fn current(&self) -> Option<Project> {
        self.0.ring.front().and_then(|k| self.0.all.get(k)).cloned()
    }

    pub fn next(&self) -> Option<Project> {
        self.0.ring.back().and_then(|k| self.0.all.get(k)).cloned()
    }

    pub fn apply(&mut self, mutation: Mutation, key: &StoreKey) {
        match mutation {
            Mutation::None => {}
            Mutation::Insert => {
                self.move_to_back(key);
                self.0.ring.rotate_right(1);
            }
            Mutation::RotateLeft => self.0.ring.rotate_left(1),
            Mutation::RotateRight => self.0.ring.rotate_right(1),
        };
    }

    pub fn open(&self) -> Vec<Project> {
        let terminal_windows = config::TERMINAL.window_names();
        self.0
            .ring
            .iter()
            .filter_map(|k| self.0.all.get(k))
            .filter(|p| terminal_windows.contains(&p.store_key().to_string()))
            .cloned()
            .collect()
    }

    pub fn add(&mut self, path: &str, name: Option<&str>) {
        let path = PathBuf::from(path.to_string());
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if Some(path.as_path()) == dirs::home_dir().as_deref() {
            return;
        }
        let name = name
            .map(|s| s.to_string())
            .unwrap_or_else(|| path.file_name().unwrap().to_str().unwrap().to_string());
        let key = StoreKey::project(&name);
        if !self.0.all.contains_key(&key) {
            ps!("projects::add");
            self.0.all.insert(
                key.clone(),
                Project {
                    repo_name: name,
                    repo_path: path,
                    kv: HashMap::new(),
                    last_application: None,
                    branch: None,
                    github_pr: None,
                    github_repo: None,
                },
            );
            self.0.ring.push_front(key);
        }
    }

    pub fn add_project(&mut self, project: Project) {
        if Some(project.repo_path.as_path()) == dirs::home_dir().as_deref() {
            return;
        }
        let key = project.store_key();
        if !self.0.all.contains_key(&key) {
            ps!("projects::add_project");
            self.0.all.insert(key.clone(), project);
        }
        if !self.0.ring.contains(&key) {
            self.0.ring.push_front(key);
        }
    }

    pub fn remove(&mut self, key: &StoreKey) -> bool {
        if self.0.all.remove(key).is_some() {
            if let Some(i) = self.ring_index(key) {
                self.0.ring.remove(i);
            }
            true
        } else {
            false
        }
    }

    pub fn remove_from_ring(&mut self, key: &StoreKey) {
        if let Some(i) = self.ring_index(key) {
            self.0.ring.remove(i);
        }
    }

    pub fn move_to_back(&mut self, key: &StoreKey) {
        if let Some(i) = self.ring_index(key) {
            if let Some(k) = self.0.ring.remove(i) {
                self.0.ring.push_back(k);
            }
        }
    }

    pub fn set_last_application(&mut self, key: &StoreKey, application: Application) {
        if let Some(p) = self.0.all.get_mut(key) {
            p.last_application = Some(application);
        }
    }

    pub fn by_path(&self, query_path: &Path) -> Option<Project> {
        let query_path =
            std::fs::canonicalize(query_path).unwrap_or_else(|_| query_path.to_path_buf());
        self.0
            .all
            .values()
            .filter(|p| {
                query_path.starts_with(&p.repo_path)
                    || p.worktree_path()
                        .map(|wt| query_path.starts_with(&wt))
                        .unwrap_or(false)
            })
            .max_by_key(|p| {
                p.worktree_path()
                    .unwrap_or_else(|| p.repo_path.clone())
                    .as_os_str()
                    .len()
            })
            .cloned()
    }

    pub fn by_exact_path(&self, path: &Path) -> Option<Project> {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.0.all.values().find(|p| p.repo_path == path).cloned()
    }

    pub fn by_key(&self, key: &StoreKey) -> Option<Project> {
        self.0.all.get(key).cloned()
    }

    pub fn get_mut(&mut self, key: &StoreKey) -> Option<&mut Project> {
        self.0.all.get_mut(key)
    }

    fn ring_index(&self, key: &StoreKey) -> Option<usize> {
        self.0.ring.iter().position(|k| k == key)
    }

    pub fn print(&self) {
        let previous = self
            .previous()
            .map(|p| p.repo_name)
            .unwrap_or("none".into());
        let current = self.current().map(|p| p.repo_name).unwrap_or("none".into());
        let next = self.next().map(|p| p.repo_name).unwrap_or("none".into());
        let len = self.0.ring.len();

        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            println!("{}", execute_command("vscode-summary", [], "/tmp"));
            println!();
            ps!("..., {}, {}*, {}, ... ({})", previous, current, next, len,);
        });
    }
}

pub fn load() {
    let mut projects = lock();

    // First, discover all tasks (worktrees) from known project paths
    let tasks = discover_tasks(HashMap::new());
    for (key, project) in tasks {
        if !projects.0.ring.contains(&key) {
            projects.0.ring.push_back(key.clone());
        }
        projects.0.all.insert(key, project);
    }

    // Build a reverse map from canonical path to disambiguated name
    let available = config::available_projects();
    let path_to_name: HashMap<PathBuf, String> = available
        .into_iter()
        .filter_map(|(name, path)| {
            std::fs::canonicalize(&path)
                .ok()
                .map(|canonical| (canonical, name))
        })
        .collect();

    // Load projects from terminal state into the ring
    let home_dir = dirs::home_dir();
    for dir in config::TERMINAL.project_directories() {
        let path = PathBuf::from(&dir);
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if Some(canonical.as_path()) == home_dir.as_deref() {
            continue;
        }

        let name = path_to_name.get(&canonical).cloned().unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        let key = StoreKey::project(&name);

        // Add to all if not already present
        if !projects.0.all.contains_key(&key) {
            projects.0.all.insert(
                key.clone(),
                Project {
                    repo_name: name,
                    repo_path: canonical,
                    kv: HashMap::new(),
                    last_application: None,
                    branch: None,
                    github_pr: None,
                    github_repo: None,
                },
            );
        }

        // Add to ring if not already present
        if !projects.0.ring.contains(&key) {
            projects.0.ring.push_back(key);
        }
    }

    crate::kv::load_kv_data(&mut projects);
    if crate::util::debug() {
        projects.print();
    }
}

fn discover_tasks(additional_paths: HashMap<String, PathBuf>) -> HashMap<StoreKey, Project> {
    let mut project_paths: HashMap<String, PathBuf> =
        config::available_projects().into_iter().collect();

    for (name, path) in additional_paths {
        project_paths.entry(name).or_insert(path);
    }

    project_paths
        .into_par_iter()
        .flat_map(|(project_name, project_path)| {
            if !git::is_git_repo(&project_path) {
                return vec![];
            }
            let worktrees_dir = git::worktree_base_path(&project_path);
            git::list_worktrees(&project_path)
                .into_iter()
                .filter(|wt| wt.path.starts_with(&worktrees_dir))
                .filter_map(|wt| {
                    let branch = wt.branch.as_ref()?;
                    let task = Project {
                        repo_name: project_name.clone(),
                        repo_path: project_path.clone(),
                        kv: HashMap::new(),
                        last_application: None,
                        branch: Some(branch.clone()),
                        github_pr: None,
                        github_repo: None,
                    };
                    Some((task.store_key(), task))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn refresh_tasks() {
    let additional_paths: HashMap<String, PathBuf> = {
        let store = STORE.lock().unwrap();
        store
            .all
            .iter()
            .filter(|(_, p)| !p.is_task())
            .map(|(key, project)| (key.repo.clone(), project.repo_path.clone()))
            .collect()
    };

    let tasks = discover_tasks(additional_paths);

    let mut projects = lock();
    for (key, project) in tasks {
        projects.0.all.entry(key).or_insert(project);
    }
}

pub fn tasks() -> HashMap<StoreKey, Project> {
    let projects = lock();
    projects
        .0
        .all
        .iter()
        .filter(|(_, p)| p.is_task())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
