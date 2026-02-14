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

    let worktree_preexisted = worktree_path.exists();
    if !worktree_preexisted {
        git::create_worktree(&repo_path, &worktree_path, branch)?;
        setup_task_worktree(&worktree_path, repo, branch)?;
    }

    // Refresh to pick up the new task
    projects::refresh_tasks();

    let task = get_task_by_branch(repo, branch).ok_or_else(|| {
        diagnose_task_not_found(
            repo,
            branch,
            &repo_path,
            &worktree_path,
            worktree_preexisted,
        )
    })?;

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

pub fn setup_task_worktree(worktree_path: &Path, repo: &str, branch: &str) -> Result<(), String> {
    let task_dir = worktree_path.join(".task");
    fs::create_dir_all(&task_dir)
        .map_err(|e| format!("Failed to create .task directory: {}", e))?;

    ensure_gitattributes_entry(worktree_path)?;

    let project_key = format!("{}:{}", repo, branch);
    let content = format!(
        concat!(
            "At the start of the conversation output the following ",
            "so that I know you've read these instructions:\n",
            "\n",
            "\u{1F4D6} {}\n",
        ),
        project_key
    );
    let agents_path = task_dir.join("AGENTS.md");
    if !agents_path.exists() {
        fs::write(&agents_path, &content)
            .map_err(|e| format!("Failed to create .task/AGENTS.md: {}", e))?;
    }

    let target = Path::new(".task/AGENTS.md");
    create_agent_symlink(worktree_path, "CLAUDE.md", target)?;
    create_agent_symlink(worktree_path, "AGENTS.md", target)?;
    Ok(())
}

fn create_agent_symlink(worktree_path: &Path, filename: &str, target: &Path) -> Result<(), String> {
    let link_path = worktree_path.join(filename);
    if link_path.symlink_metadata().is_ok() {
        if link_path.read_link().ok().as_deref() == Some(target) {
            return Ok(());
        }
        // Mark as assume-unchanged before replacing (no-op if untracked)
        let _ = std::process::Command::new("git")
            .args(["update-index", "--assume-unchanged", filename])
            .current_dir(worktree_path)
            .output();
        fs::remove_file(&link_path).map_err(|e| format!("Failed to remove {}: {}", filename, e))?;
    }
    std::os::unix::fs::symlink(target, &link_path)
        .map_err(|e| format!("Failed to create {} symlink: {}", filename, e))
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

fn diagnose_task_not_found(
    repo: &str,
    branch: &str,
    repo_path: &Path,
    worktree_path: &Path,
    worktree_preexisted: bool,
) -> String {
    let path = worktree_path.display();
    let worktrees = git::list_worktrees(repo_path);
    let git_knows = worktrees.iter().find(|wt| wt.path == worktree_path);

    if worktree_preexisted {
        match git_knows {
            None => format!(
                "Directory {} exists but is not a git worktree. \
                 Remove it or run `git worktree prune` and retry.",
                path
            ),
            Some(wt) => format!(
                "Git worktree exists at {} with branch {:?}, \
                 but expected branch '{}'. \
                 Remove it or check it out on the correct branch.",
                path, wt.branch, branch
            ),
        }
    } else {
        format!(
            "Created git worktree at {} but it was not discovered as task '{}:{}'. \
             This is a bug.",
            path, repo, branch
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_task_worktree_preserves_existing_agents_md() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path();

        // First call seeds .task/AGENTS.md
        setup_task_worktree(worktree, "repo", "branch").unwrap();
        let seeded = fs::read_to_string(worktree.join(".task/AGENTS.md")).unwrap();
        assert!(seeded.contains("repo:branch"));

        // Write custom content
        fs::write(worktree.join(".task/AGENTS.md"), "# Custom\n").unwrap();

        // Second call should not overwrite
        setup_task_worktree(worktree, "repo", "branch").unwrap();
        let preserved = fs::read_to_string(worktree.join(".task/AGENTS.md")).unwrap();
        assert_eq!(preserved, "# Custom\n");
    }
}

fn parse_land_in(s: Option<&String>) -> Option<Application> {
    s.and_then(|v| match v.as_str() {
        "terminal" => Some(Application::Terminal),
        "editor" => Some(Application::Editor),
        _ => None,
    })
}
