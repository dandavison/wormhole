use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use crate::project::ProjectKey;
use crate::wormhole::Application;
use crate::{config, editor, git, project::Project, projects, util::warn};

pub fn get_task(key: &ProjectKey) -> Option<Project> {
    let projects = projects::lock();
    projects.by_key(key).filter(|p| p.is_task())
}

/// Get a task by repo and branch
pub fn get_task_by_branch(repo: &str, branch: &str) -> Option<Project> {
    get_task(&ProjectKey::task(repo, branch))
}

/// Create a task. The branch name is the task identity.
pub fn create_task(repo: &str, branch: &str) -> Result<Project, String> {
    if let Some(task) = get_task_by_branch(repo, branch) {
        return Ok(task);
    }

    let repo_path = resolve_project_path(repo)?;

    if !git::is_git_repo(&repo_path) {
        return Err(format!("'{}' is not a git repository", repo));
    }

    let worktree_path = git::worktree_base_path(&repo_path)
        .join(git::encode_branch_for_path(branch))
        .join(repo);

    if !worktree_path.exists() {
        git::create_worktree(&repo_path, &worktree_path, branch)?;
        setup_task_directory(&worktree_path)?;
    }

    // Refresh to pick up the new task
    projects::refresh_tasks();

    let task = get_task_by_branch(repo, branch)
        .ok_or_else(|| format!("Failed to create task '{}:{}'", repo, branch))?;

    // Add to ring so it appears in project list
    {
        let mut projects = projects::lock();
        projects.add_project(task.clone());
        projects.apply(projects::Mutation::Insert, &task.store_key());
    }

    Ok(task)
}

pub fn open_task(
    repo: &str,
    branch: &str,
    land_in: Option<Application>,
    skip_editor: bool,
    focus_terminal: bool,
) -> Result<(), String> {
    let project = if let Some(task) = get_task_by_branch(repo, branch) {
        task
    } else {
        create_task(repo, branch)?
    };

    {
        let mut projects = projects::lock();
        projects.add_project(project.clone());
        projects.apply(projects::Mutation::Insert, &project.store_key());
    }

    let open_terminal = {
        let project = project.clone();
        move || {
            config::TERMINAL.open(&project).unwrap_or_else(|err| {
                warn(&format!(
                    "Error opening {} in terminal: {}",
                    &project.repo_name, err
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

    if skip_editor {
        open_terminal();
        if focus_terminal {
            config::TERMINAL.focus();
        }
    } else {
        let land_in = land_in.or_else(|| parse_land_in(project.kv.get("land-in")));
        match land_in {
            Some(Application::Terminal) => {
                open_terminal();
                open_editor();
                config::TERMINAL.focus();
            }
            Some(Application::Editor) => {
                open_editor();
                config::editor().focus();
                open_terminal();
            }
            None => {
                let terminal_thread = thread::spawn(open_terminal);
                let editor_thread = thread::spawn(open_editor);
                terminal_thread.join().unwrap();
                editor_thread.join().unwrap();
                config::editor().focus();
            }
        }
    }

    Ok(())
}

pub fn remove_task(repo: &str, branch: &str) -> Result<(), String> {
    let task = get_task_by_branch(repo, branch)
        .ok_or_else(|| format!("Task '{}:{}' not found", repo, branch))?;

    let worktree_path = task
        .worktree_path()
        .ok_or_else(|| format!("'{}:{}' is not a task", repo, branch))?;

    crate::serve_web::manager().stop(&task.store_key().to_string());
    git::remove_worktree(&task.repo_path, &worktree_path)?;

    // Delete KV file for this task
    crate::kv::delete_kv_file(&task);

    // Remove from unified store
    {
        let mut projects = projects::lock();
        projects.remove(&task.store_key());
    }

    Ok(())
}

fn resolve_project_path(project_name: &str) -> Result<PathBuf, String> {
    config::resolve_project_name(project_name)
        .or_else(|| {
            projects::lock()
                .by_key(&ProjectKey::project(project_name))
                .map(|p| p.repo_path.clone())
        })
        .ok_or_else(|| format!("Project '{}' not found", project_name))
}

fn setup_task_directory(worktree_path: &Path) -> Result<(), String> {
    let task_dir = worktree_path.join(".task");
    fs::create_dir_all(&task_dir)
        .map_err(|e| format!("Failed to create .task directory: {}", e))?;

    fs::write(task_dir.join("plan.md"), "")
        .map_err(|e| format!("Failed to create plan.md: {}", e))?;

    ensure_gitattributes_entry(worktree_path)?;
    Ok(())
}

fn ensure_gitattributes_entry(worktree_path: &Path) -> Result<(), String> {
    let gitattributes_path = worktree_path.join(".gitattributes");
    let entry = ".task/ linguist-generated";

    let contents = fs::read_to_string(&gitattributes_path).unwrap_or_default();
    if contents.lines().any(|line| line.trim() == entry) {
        return Ok(());
    }

    let new_contents = if contents.is_empty() || contents.ends_with('\n') {
        format!("{}{}\n", contents, entry)
    } else {
        format!("{}\n{}\n", contents, entry)
    };

    fs::write(&gitattributes_path, new_contents)
        .map_err(|e| format!("Failed to update .gitattributes: {}", e))
}

fn parse_land_in(s: Option<&String>) -> Option<Application> {
    s.and_then(|v| match v.as_str() {
        "terminal" => Some(Application::Terminal),
        "editor" => Some(Application::Editor),
        _ => None,
    })
}
