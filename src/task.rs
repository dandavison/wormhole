use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use crate::wormhole::Application;
use crate::{config, editor, git, project::Project, projects, util::warn};

pub fn get_task(id: &str) -> Option<Project> {
    let projects = projects::lock();
    projects.by_name(id).filter(|p| p.home_project.is_some())
}

pub fn task_by_path(path: &Path) -> Option<Project> {
    let path = std::fs::canonicalize(path).ok()?;
    let projects = projects::lock();
    projects
        .all()
        .into_iter()
        .filter(|p| p.home_project.is_some())
        .find(|p| path.starts_with(&p.path))
        .cloned()
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
        setup_task_directory(&worktree_path)?;
    }

    // Refresh to pick up the new task
    projects::refresh_tasks();

    get_task(task_id).ok_or_else(|| format!("Failed to create task '{}'", task_id))
}

pub fn open_task(
    task_id: &str,
    home: Option<&str>,
    branch: Option<&str>,
    land_in: Option<Application>,
    skip_editor: bool,
    focus_terminal: bool,
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
        projects.add_project(project.clone());
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

    crate::serve_web::manager().stop(task_id);
    git::remove_worktree(&home_path, &task.path)?;

    // Remove from unified store
    {
        let mut projects = projects::lock();
        projects.remove(task_id);
    }

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
