use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use std::thread;

use rayon::prelude::*;

use crate::wormhole::Application;
use crate::{config, editor, git, project::Project, projects, util::warn};

static TASK_CACHE: RwLock<Option<HashMap<String, Project>>> = RwLock::new(None);

pub fn discover_tasks() -> HashMap<String, Project> {
    let mut project_name_to_path: HashMap<String, PathBuf> =
        config::available_projects().into_iter().collect();
    for project in projects::lock().all() {
        project_name_to_path
            .entry(project.name.clone())
            .or_insert_with(|| project.path.clone());
    }

    project_name_to_path
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
                    let task_id = wt.path.file_name()?.to_str()?;
                    if task_id == project_name {
                        return None;
                    }
                    Some((
                        task_id.to_string(),
                        Project {
                            name: task_id.to_string(),
                            path: wt.path,
                            aliases: vec![],
                            kv: HashMap::new(),
                            last_application: None,
                            home_project: Some(project_name.clone()),
                        },
                    ))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn tasks() -> HashMap<String, Project> {
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

fn refresh_cache() -> HashMap<String, Project> {
    let tasks = discover_tasks();
    let mut cache = TASK_CACHE.write().unwrap();
    *cache = Some(tasks.clone());
    tasks
}

pub fn get_task(id: &str) -> Option<Project> {
    tasks().get(id).cloned()
}

pub fn create_task(task_id: &str, home: &str, branch: Option<&str>) -> Result<Project, String> {
    if let Some(task) = get_task(task_id) {
        return Ok(task);
    }

    let home_path = resolve_project_path(home)?;

    if !git::is_git_repo(&home_path) {
        return Err(format!("'{}' is not a git repository", home));
    }

    let worktree_path = git::worktree_base_path(&home_path).join(task_id);
    let branch_name = branch.unwrap_or(task_id);

    if !worktree_path.exists() {
        git::create_worktree(&home_path, &worktree_path, branch_name)?;
    }

    refresh_cache();

    Ok(Project {
        name: task_id.to_string(),
        path: worktree_path,
        aliases: vec![],
        kv: HashMap::new(),
        last_application: None,
        home_project: Some(home.to_string()),
    })
}

pub fn open_task(
    task_id: &str,
    home: Option<&str>,
    branch: Option<&str>,
    land_in: Option<Application>,
) -> Result<(), String> {
    let project = if let Some(task) = get_task(task_id) {
        task
    } else {
        let home = home
            .ok_or_else(|| format!("Task '{}' not found. Specify --home to create it.", task_id))?;
        create_task(task_id, home, branch)?
    };

    {
        let mut projects = projects::lock();
        if projects.by_name(&project.name).is_none() {
            projects.add_project(project.clone());
        }
        projects.apply(projects::Mutation::Insert, &project.name);
    }

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

    Ok(())
}

pub fn remove_task(task_id: &str) -> Result<(), String> {
    let task = get_task(task_id).ok_or_else(|| format!("Task '{}' not found", task_id))?;
    let home = task
        .home_project
        .as_ref()
        .ok_or_else(|| format!("'{}' is not a task", task_id))?;
    let home_path = resolve_project_path(home)?;

    git::remove_worktree(&home_path, &task.path)?;
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
