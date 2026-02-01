use crate::project::{BranchName, Cached, Project, ProjectKey, RepoName};
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

struct ProjectsStore {
    all: HashMap<ProjectKey, Project>,
    ring: VecDeque<ProjectKey>,
}

lazy_static! {
    static ref PROJECTS_STORE: Mutex<ProjectsStore> = Mutex::new(ProjectsStore {
        all: HashMap::new(),
        ring: VecDeque::new(),
    });
}

pub struct Projects<'a>(MutexGuard<'a, ProjectsStore>);

pub fn lock<'a>() -> Projects<'a> {
    Projects(PROJECTS_STORE.lock().unwrap())
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

    pub fn keys(&self) -> Vec<ProjectKey> {
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

    pub fn apply(&mut self, mutation: Mutation, key: &ProjectKey) {
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
            .filter(|p| {
                // Tasks always appear, projects only if they have terminal windows
                p.is_task() || terminal_windows.contains(&p.store_key().to_string())
            })
            .cloned()
            .collect()
    }

    pub fn add(&mut self, path: &str, name: Option<&str>) -> Result<(), String> {
        let path = PathBuf::from(path.to_string());
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if Some(path.as_path()) == dirs::home_dir().as_deref() {
            return Err("Cannot add home directory as a project".to_string());
        }
        if !git::is_git_repo(&path) {
            return Err(format!("'{}' is not a git repository", path.display()));
        }
        let name = name
            .map(|s| s.to_string())
            .unwrap_or_else(|| path.file_name().unwrap().to_str().unwrap().to_string());
        let key = ProjectKey::project(&name);
        if !self.0.all.contains_key(&key) {
            ps!("projects::add");
            let git_common_dir = git::git_common_dir(&path);
            self.0.all.insert(
                key.clone(),
                Project {
                    repo_name: RepoName::new(name),
                    repo_path: path,
                    branch: None,
                    kv: HashMap::new(),
                    last_application: None,
                    cached: Cached {
                        git_common_dir: Some(git_common_dir),
                        ..Default::default()
                    },
                },
            );
            self.0.ring.push_front(key);
        }
        Ok(())
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

    pub fn remove(&mut self, key: &ProjectKey) -> bool {
        if self.0.all.remove(key).is_some() {
            if let Some(i) = self.ring_index(key) {
                self.0.ring.remove(i);
            }
            true
        } else {
            false
        }
    }

    pub fn remove_from_ring(&mut self, key: &ProjectKey) {
        if let Some(i) = self.ring_index(key) {
            self.0.ring.remove(i);
        }
    }

    pub fn move_to_back(&mut self, key: &ProjectKey) {
        if let Some(i) = self.ring_index(key) {
            if let Some(k) = self.0.ring.remove(i) {
                self.0.ring.push_back(k);
            }
        }
    }

    pub fn set_last_application(&mut self, key: &ProjectKey, application: Application) {
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
                // For tasks, only match if path is inside the worktree.
                // For non-tasks, match if path is inside repo_path.
                if let Some(wt) = p.worktree_path() {
                    query_path.starts_with(&wt)
                } else {
                    query_path.starts_with(&p.repo_path)
                }
            })
            .max_by_key(|p| p.working_tree().as_os_str().len())
            .cloned()
    }

    pub fn by_key(&self, key: &ProjectKey) -> Option<Project> {
        self.0.all.get(key).cloned()
    }

    pub fn get_mut(&mut self, key: &ProjectKey) -> Option<&mut Project> {
        self.0.all.get_mut(key)
    }

    fn ring_index(&self, key: &ProjectKey) -> Option<usize> {
        self.0.ring.iter().position(|k| k == key)
    }

    pub fn print(&self) {
        let previous = self
            .previous()
            .map(|p| p.repo_name.to_string())
            .unwrap_or_else(|| "none".to_string());
        let current = self
            .current()
            .map(|p| p.repo_name.to_string())
            .unwrap_or_else(|| "none".to_string());
        let next = self
            .next()
            .map(|p| p.repo_name.to_string())
            .unwrap_or_else(|| "none".to_string());
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

        // Skip worktrees - they're already handled by discover_tasks
        if git::is_worktree(&path) {
            continue;
        }

        let name = path_to_name.get(&canonical).cloned().unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        let key = ProjectKey::project(&name);

        // Add to all if not already present
        if !projects.0.all.contains_key(&key) {
            let git_common_dir = git::git_common_dir(&canonical);
            projects.0.all.insert(
                key.clone(),
                Project {
                    repo_name: RepoName::new(name),
                    repo_path: canonical,
                    branch: None,
                    kv: HashMap::new(),
                    last_application: None,
                    cached: Cached {
                        git_common_dir: Some(git_common_dir),
                        ..Default::default()
                    },
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

fn discover_tasks(additional_paths: HashMap<String, PathBuf>) -> HashMap<ProjectKey, Project> {
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
            let git_common_dir = git::git_common_dir(&project_path);
            let worktrees_dir = git_common_dir.join("wormhole/worktrees");
            git::list_worktrees(&project_path)
                .into_iter()
                .filter(|wt| wt.path.starts_with(&worktrees_dir))
                .filter_map(|wt| {
                    let branch = wt.branch.as_ref()?;
                    let task = Project {
                        repo_name: RepoName::new(project_name.clone()),
                        repo_path: project_path.clone(),
                        branch: Some(BranchName::new(branch.clone())),
                        kv: HashMap::new(),
                        last_application: None,
                        cached: Cached {
                            git_common_dir: Some(git_common_dir.clone()),
                            ..Default::default()
                        },
                    };
                    Some((task.store_key(), task))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn refresh_tasks() {
    let additional_paths: HashMap<String, PathBuf> = {
        let store = PROJECTS_STORE.lock().unwrap();
        store
            .all
            .iter()
            .filter(|(_, p)| !p.is_task())
            .map(|(key, project)| (key.repo.to_string(), project.repo_path.clone()))
            .collect()
    };

    let tasks = discover_tasks(additional_paths);

    let mut projects = lock();
    for (key, project) in tasks {
        // Add to ring if not already present (so tasks appear in project list)
        if !projects.0.ring.contains(&key) {
            projects.0.ring.push_back(key.clone());
        }
        projects.0.all.entry(key).or_insert(project);
    }
}

pub fn tasks() -> HashMap<ProjectKey, Project> {
    let projects = lock();
    projects
        .0
        .all
        .iter()
        .filter(|(_, p)| p.is_task())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn refresh_cache() {
    use crate::{github, jira};

    let task_info: Vec<_> = {
        let projects = lock();
        projects
            .0
            .all
            .iter()
            .filter(|(_, p)| p.is_task())
            .map(|(key, p)| {
                let jira_key = p.kv.get("jira_key").cloned();
                let path = p.working_tree();
                (key.clone(), jira_key, path)
            })
            .collect()
    };

    let results: Vec<_> = task_info
        .par_iter()
        .map(|(key, jira_key, path)| {
            let jira = jira_key
                .as_ref()
                .and_then(|k| jira::get_issue(k).ok().flatten());
            let pr = github::get_pr_status(path);
            (key.clone(), jira, pr)
        })
        .collect();

    let mut projects = lock();
    for (key, jira, pr) in results {
        if let Some(project) = projects.0.all.get_mut(&key) {
            project.cached.jira = jira;
            project.cached.pr = pr;
        }
    }
}

pub fn cache_needs_refresh() -> bool {
    if std::env::var("WORMHOLE_OFFLINE").is_ok() {
        return false;
    }
    let projects = lock();
    projects
        .0
        .all
        .iter()
        .filter(|(_, p)| p.is_task())
        .any(|(_, p)| {
            let has_jira_key = p.kv.contains_key("jira_key");
            let jira_missing = has_jira_key && p.cached.jira.is_none();
            let pr_missing = p.cached.pr.is_none();
            jira_missing || pr_missing
        })
}
