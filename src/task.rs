use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use std::thread;

use crate::wormhole::Application;
use crate::{config, editor, git, project::Project, projects, util::warn};

#[derive(Clone, Debug)]
pub struct Task {
    pub id: String,
    pub project_name: String,
    pub worktree_path: PathBuf,
}

static TASK_CACHE: RwLock<Option<HashMap<String, Task>>> = RwLock::new(None);

pub fn discover_tasks() -> HashMap<String, Task> {
    let mut tasks = HashMap::new();

    let mut project_name_to_path: HashMap<String, PathBuf> =
        config::available_projects().into_iter().collect();
    for project in projects::lock().all() {
        project_name_to_path
            .entry(project.name.clone())
            .or_insert_with(|| project.path.clone());
    }

    for (project_name, project_path) in project_name_to_path {
        if !git::is_git_repo(&project_path) {
            continue;
        }

        let worktrees_dir = git::worktree_base_path(&project_path);

        for worktree in git::list_worktrees(&project_path) {
            if !worktree.path.starts_with(&worktrees_dir) {
                continue;
            }

            if let Some(task_id) = worktree.path.file_name().and_then(|n| n.to_str()) {
                tasks.insert(
                    task_id.to_string(),
                    Task {
                        id: task_id.to_string(),
                        project_name: project_name.clone(),
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
    project_name: Option<&str>,
    land_in: Option<Application>,
) -> Result<(), String> {
    let task = if let Some(task) = get_task(task_id) {
        task
    } else {
        let project_name = project_name
            .ok_or_else(|| format!("Task '{}' not found. Specify --home to create it.", task_id))?;

        let project_path = resolve_project_path(project_name)?;

        if !git::is_git_repo(&project_path) {
            return Err(format!("'{}' is not a git repository", project_name));
        }

        let worktree_path = git::worktree_base_path(&project_path).join(task_id);

        if !worktree_path.exists() {
            git::create_worktree(&project_path, &worktree_path, task_id)?;
        }

        refresh_cache();

        Task {
            id: task_id.to_string(),
            project_name: project_name.to_string(),
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
                warn(&format!(
                    "Error opening {} in terminal: {}",
                    &project.name, err
                ))
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

    let mut projects = projects::lock();
    if projects.by_name(&task.id).is_none() {
        projects.add(&task.worktree_path.to_string_lossy(), vec![task.id.clone()]);
    }

    Ok(())
}

pub fn remove_task(task_id: &str) -> Result<(), String> {
    let task = get_task(task_id).ok_or_else(|| format!("Task '{}' not found", task_id))?;
    let project_path = resolve_project_path(&task.project_name)?;

    git::remove_worktree(&project_path, &task.worktree_path)?;
    refresh_cache();

    Ok(())
}

fn resolve_project_path(project_name: &str) -> Result<PathBuf, String> {
    config::resolve_project_name(project_name)
        .or_else(|| {
            projects::lock()
                .by_name(project_name)
                .map(|p| p.path.clone())
        })
        .ok_or_else(|| format!("Project '{}' not found", project_name))
}
