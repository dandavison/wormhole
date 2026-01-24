use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use std::thread;

use crate::wormhole::Application;
use crate::{config, editor, git, project::Project, util::warn};

#[derive(Clone, Debug)]
pub struct Task {
    pub id: String,
    pub home_repo: PathBuf,
    pub worktree_path: PathBuf,
}

static TASK_CACHE: RwLock<Option<HashMap<String, Task>>> = RwLock::new(None);

pub fn discover_tasks() -> HashMap<String, Task> {
    let mut tasks = HashMap::new();

    for project_path in config::available_projects().values() {
        if !git::is_git_repo(project_path) {
            continue;
        }

        let worktrees_dir = git::worktree_base_path(project_path);

        for worktree in git::list_worktrees(project_path) {
            if !worktree.path.starts_with(&worktrees_dir) {
                continue;
            }

            if let Some(task_id) = worktree.path.file_name().and_then(|n| n.to_str()) {
                tasks.insert(
                    task_id.to_string(),
                    Task {
                        id: task_id.to_string(),
                        home_repo: project_path.clone(),
                        worktree_path: worktree.path,
                    },
                );
            }
        }
    }

    tasks
}

fn get_cached_tasks() -> HashMap<String, Task> {
    let cache = TASK_CACHE.read().unwrap();
    if let Some(tasks) = cache.as_ref() {
        return tasks.clone();
    }
    drop(cache);

    let tasks = discover_tasks();
    let mut cache = TASK_CACHE.write().unwrap();
    *cache = Some(tasks.clone());
    tasks
}

fn refresh_cache() -> HashMap<String, Task> {
    let tasks = discover_tasks();
    let mut cache = TASK_CACHE.write().unwrap();
    *cache = Some(tasks.clone());
    tasks
}

pub fn get_task(id: &str) -> Option<Task> {
    let tasks = get_cached_tasks();
    if let Some(task) = tasks.get(id) {
        return Some(task.clone());
    }

    let tasks = refresh_cache();
    tasks.get(id).cloned()
}

pub fn list_tasks() -> Vec<Task> {
    get_cached_tasks().into_values().collect()
}

pub fn open_task(
    task_id: &str,
    home_repo_name: Option<&str>,
    land_in: Option<Application>,
) -> Result<(), String> {
    let task = if let Some(task) = get_task(task_id) {
        task
    } else {
        let home_repo_name = home_repo_name.ok_or_else(|| {
            format!(
                "Task '{}' not found. Specify --home to create it.",
                task_id
            )
        })?;

        let home_repo_path = config::resolve_project_name(home_repo_name)
            .ok_or_else(|| format!("Home repo '{}' not found", home_repo_name))?;

        if !git::is_git_repo(&home_repo_path) {
            return Err(format!("'{}' is not a git repository", home_repo_name));
        }

        let worktree_path = git::worktree_base_path(&home_repo_path).join(task_id);

        if !worktree_path.exists() {
            git::create_worktree(&home_repo_path, &worktree_path, task_id)?;
        }

        refresh_cache();

        Task {
            id: task_id.to_string(),
            home_repo: home_repo_path,
            worktree_path,
        }
    };

    let project = Project {
        name: task.id.clone(),
        path: task.worktree_path.clone(),
        aliases: vec![],
        kv: std::collections::HashMap::new(),
        last_application: None,
    };

    let open_terminal = {
        let project = project.clone();
        move || {
            config::TERMINAL.open(&project).unwrap_or_else(|err| {
                warn(&format!("Error opening {} in terminal: {}", &project.name, err))
            })
        }
    };

    let open_editor = {
        let project = project.clone();
        move || {
            editor::open_workspace(&project);
        }
    };

    match land_in {
        Some(Application::Terminal) => {
            open_terminal();
            config::TERMINAL.focus();
            open_editor();
        }
        Some(Application::Editor) => {
            open_editor();
            config::EDITOR.focus();
            open_terminal();
        }
        None => {
            let terminal_thread = thread::spawn(open_terminal);
            let editor_thread = thread::spawn(open_editor);
            terminal_thread.join().unwrap();
            editor_thread.join().unwrap();
            config::EDITOR.focus();
        }
    }

    Ok(())
}

pub fn delete_task(task_id: &str) -> Result<(), String> {
    let task = get_task(task_id)
        .ok_or_else(|| format!("Task '{}' not found", task_id))?;

    git::remove_worktree(&task.home_repo, &task.worktree_path)?;

    if let Err(e) = git::delete_branch(&task.home_repo, task_id) {
        warn(&format!("Could not delete branch {}: {}", task_id, e));
    }

    refresh_cache();

    Ok(())
}
