use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;

use hyper::{Body, Request, Response, StatusCode};
use lazy_static::lazy_static;
use serde::Deserialize;

use crate::project::ProjectKey;
use crate::wormhole::Application;
use crate::{batch, config, editor, git, project::Project, projects, util::warn};

lazy_static! {
    static ref AGENT_BATCHES: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

/// Look up the current agent batch ID for a task (if any, and still exists).
pub fn agent_batch_id(task: &str) -> Option<String> {
    let map = AGENT_BATCHES.lock().unwrap();
    let batch_id = map.get(task)?;
    let store = batch::lock();
    store.get(batch_id).map(|_| batch_id.clone())
}

#[derive(Deserialize)]
struct NotifyAgentRequest {
    task: String,
    prompt: String,
}

/// HTTP handler for POST /task/notify-agent
pub async fn notify_agent(req: Request<Body>) -> Response<Body> {
    let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();
    let request: Result<NotifyAgentRequest, _> = serde_json::from_slice(&body_bytes);
    let request = match request {
        Ok(r) => r,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Invalid JSON: {}", e)))
                .unwrap();
        }
    };

    // Check concurrency: one agent per task
    {
        let map = AGENT_BATCHES.lock().unwrap();
        if let Some(batch_id) = map.get(&request.task) {
            let store = batch::lock();
            if let Some(b) = store.get(batch_id) {
                if !b.is_done() {
                    let json = serde_json::json!({
                        "status": "running",
                        "batch_id": batch_id,
                        "agent": crate::agent::agent_name(),
                    });
                    return Response::builder()
                        .status(StatusCode::CONFLICT)
                        .header("Content-Type", "application/json")
                        .body(Body::from(json.to_string()))
                        .unwrap();
                }
            }
        }
    }

    // Look up task to get worktree path
    let key = ProjectKey::parse(&request.task);
    let project = {
        let projects = projects::lock();
        projects.by_key(&key)
    };
    let project = match project {
        Some(p) => p,
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!("Task not found: {}", request.task)))
                .unwrap();
        }
    };
    let dir = project.working_tree();

    // Create batch-of-1
    let batch_id = batch::create_batch(batch::BatchRequest {
        command: crate::agent::agent_command(&request.prompt),
        runs: vec![batch::RunSpec {
            key: request.task.clone(),
            dir,
        }],
    });
    batch::spawn_batch(&batch_id);

    // Record for concurrency tracking
    {
        let mut map = AGENT_BATCHES.lock().unwrap();
        map.insert(request.task, batch_id.clone());
    }

    let json = serde_json::json!({
        "status": "running",
        "batch_id": batch_id,
        "agent": crate::agent::agent_name(),
    });
    Response::builder()
        .header("Content-Type", "application/json")
        .body(Body::from(json.to_string()))
        .unwrap()
}

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

#[derive(serde::Serialize)]
pub struct ReviewTaskResult {
    pub created: Vec<String>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

pub fn create_review_tasks(dry_run: bool) -> Result<ReviewTaskResult, String> {
    projects::refresh_tasks();

    let repo_map = build_github_repo_map();
    let existing_tasks: std::collections::HashSet<String> = {
        let projects = projects::lock();
        projects
            .all()
            .iter()
            .filter(|p| p.is_task())
            .map(|p| p.store_key().to_string())
            .collect()
    };

    let prs = crate::github::search_review_requests()?;

    let mut result = ReviewTaskResult {
        created: Vec::new(),
        skipped: Vec::new(),
        errors: Vec::new(),
    };

    for pr in &prs {
        let (owner, repo_name) = match pr.repository.name_with_owner.split_once('/') {
            Some(pair) => pair,
            None => {
                result.errors.push(format!(
                    "#{}: invalid repo '{}'",
                    pr.number, pr.repository.name_with_owner
                ));
                continue;
            }
        };

        let home = match repo_map.get(&pr.repository.name_with_owner) {
            Some(name) => name.clone(),
            None => {
                result.skipped.push(format!(
                    "#{} {}: no local project for {}",
                    pr.number, pr.title, pr.repository.name_with_owner
                ));
                continue;
            }
        };

        let branch = match crate::github::get_pr_branch(owner, repo_name, pr.number) {
            Some(b) => b,
            None => {
                result
                    .errors
                    .push(format!("#{} {}: failed to get branch", pr.number, pr.title));
                continue;
            }
        };

        let task_key = format!("{}:{}", home, branch);
        if existing_tasks.contains(&task_key) {
            result.skipped.push(format!("{} (exists)", task_key));
            continue;
        }

        if dry_run {
            result.created.push(format!("{} (dry run)", task_key));
            continue;
        }

        match create_task(&home, &branch) {
            Ok(task) => {
                let worktree = task.working_tree();
                if let Err(e) = crate::github::pr_checkout(&worktree, pr.number) {
                    result
                        .errors
                        .push(format!("{}: gh pr checkout failed: {}", task_key, e));
                }
                write_review_agents_md(&worktree, &pr.url, &pr.title);
                let key = ProjectKey::task(&home, &branch);
                crate::kv::set_value_sync(&key, "task_type", "review");
                result.created.push(task_key);
            }
            Err(e) => {
                result.errors.push(format!("{}: {}", task_key, e));
            }
        }
    }

    if !dry_run && !result.created.is_empty() {
        projects::refresh_cache();
    }

    Ok(result)
}

fn build_github_repo_map() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for (name, path) in config::available_projects() {
        if let Some(github_repo) = git::github_repo_from_remote(&path) {
            map.insert(github_repo, name);
        }
    }
    map
}

fn write_review_agents_md(worktree_path: &Path, pr_url: &str, pr_title: &str) {
    let agents_path = worktree_path.join(".task/AGENTS.md");
    let content = format!(
        "Your task is to review this pull request:\n\n{}\n\nTitle: {}\n",
        pr_url, pr_title
    );
    let _ = fs::write(&agents_path, content);
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
    conform_task_worktree(worktree_path, repo, branch, false).map(|_| ())
}

/// Check/fix task worktree conformance. Returns list of actions taken (or
/// that would be taken if `dry_run` is true).
pub fn conform_task_worktree(
    worktree_path: &Path,
    repo: &str,
    branch: &str,
    dry_run: bool,
) -> Result<Vec<String>, String> {
    let mut actions = Vec::new();

    let task_dir = worktree_path.join(".task");
    if !task_dir.is_dir() {
        actions.push("create .task/".into());
        if !dry_run {
            fs::create_dir_all(&task_dir)
                .map_err(|e| format!("Failed to create .task directory: {}", e))?;
        }
    }

    if let Some(action) = check_gitattributes_entry(worktree_path, dry_run)? {
        actions.push(action);
    }

    let agents_path = task_dir.join("AGENTS.md");
    if !agents_path.exists() {
        actions.push("create .task/AGENTS.md".into());
        if !dry_run {
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
            fs::write(&agents_path, &content)
                .map_err(|e| format!("Failed to create .task/AGENTS.md: {}", e))?;
        }
    }

    let target = Path::new(".task/AGENTS.md");
    if let Some(action) = check_agent_symlink(worktree_path, "CLAUDE.md", target, dry_run)? {
        actions.push(action);
    }
    if let Some(action) = check_agent_symlink(worktree_path, "AGENTS.md", target, dry_run)? {
        actions.push(action);
    }

    Ok(actions)
}

/// Returns Some(action) if the symlink needed creating/fixing, None if already correct.
fn check_agent_symlink(
    worktree_path: &Path,
    filename: &str,
    target: &Path,
    dry_run: bool,
) -> Result<Option<String>, String> {
    let link_path = worktree_path.join(filename);
    if link_path.symlink_metadata().is_ok() {
        if link_path.read_link().ok().as_deref() == Some(target) {
            return Ok(None);
        }
        if !dry_run {
            let _ = std::process::Command::new("git")
                .args(["update-index", "--assume-unchanged", filename])
                .current_dir(worktree_path)
                .output();
            fs::remove_file(&link_path)
                .map_err(|e| format!("Failed to remove {}: {}", filename, e))?;
        }
    }
    let action = format!("symlink {} -> {}", filename, target.display());
    if !dry_run {
        std::os::unix::fs::symlink(target, &link_path)
            .map_err(|e| format!("Failed to create {} symlink: {}", filename, e))?;
    }
    Ok(Some(action))
}

/// Returns Some(action) if .gitattributes needed updating, None if already correct.
fn check_gitattributes_entry(
    worktree_path: &Path,
    dry_run: bool,
) -> Result<Option<String>, String> {
    let gitattributes_path = worktree_path.join(".gitattributes");
    let entry = ".task/ linguist-generated";

    let contents = fs::read_to_string(&gitattributes_path).unwrap_or_default();
    if contents.lines().any(|line| line.trim() == entry) {
        return Ok(None);
    }

    if !dry_run {
        let new_contents = if contents.is_empty() || contents.ends_with('\n') {
            format!("{}{}\n", contents, entry)
        } else {
            format!("{}\n{}\n", contents, entry)
        };
        fs::write(&gitattributes_path, new_contents)
            .map_err(|e| format!("Failed to update .gitattributes: {}", e))?;
    }

    Ok(Some(
        "add .task/ linguist-generated to .gitattributes".into(),
    ))
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

    #[test]
    fn conform_reports_actions_on_fresh_worktree() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path();

        let actions = conform_task_worktree(worktree, "repo", "branch", false).unwrap();
        assert!(actions.iter().any(|a| a.contains(".task/")));
        assert!(actions.iter().any(|a| a.contains("CLAUDE.md")));
        assert!(actions.iter().any(|a| a.contains("AGENTS.md")));
    }

    #[test]
    fn conform_dry_run_makes_no_changes() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path();

        let actions = conform_task_worktree(worktree, "repo", "branch", true).unwrap();
        assert!(!actions.is_empty());
        assert!(!worktree.join(".task").exists());
        assert!(!worktree.join("CLAUDE.md").exists());
    }

    #[test]
    fn conform_already_conformed_reports_no_actions() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path();

        conform_task_worktree(worktree, "repo", "branch", false).unwrap();
        let actions = conform_task_worktree(worktree, "repo", "branch", false).unwrap();
        assert!(
            actions.is_empty(),
            "expected no actions, got: {:?}",
            actions
        );
    }
}

fn parse_land_in(s: Option<&String>) -> Option<Application> {
    s.and_then(|v| match v.as_str() {
        "terminal" => Some(Application::Terminal),
        "editor" => Some(Application::Editor),
        _ => None,
    })
}
