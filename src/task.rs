use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use crate::project::ProjectKey;
use crate::wormhole::LandIn;
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

    let worktree_path = git::task_worktree_path(config::worktree_dir(), repo, branch);

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

pub fn open_task(repo: &str, branch: &str, land_in: Option<LandIn>) -> Result<(), String> {
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

    let land_in = land_in.or_else(|| crate::wormhole::parse_land_in(project.kv.get("land-in")));
    match land_in {
        Some(LandIn::TerminalOnly) => {
            open_terminal();
            config::TERMINAL.focus();
        }
        Some(LandIn::Background) => {
            open_terminal();
        }
        Some(LandIn::Terminal) => {
            open_terminal();
            open_editor();
            config::TERMINAL.focus();
        }
        Some(LandIn::Editor) => {
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
        let already_exists = existing_tasks.contains(&task_key);

        if dry_run {
            let label = if already_exists { "update" } else { "create" };
            result
                .created
                .push(format!("{} (dry run: {})", task_key, label));
            continue;
        }

        match create_task(home.as_str(), &branch) {
            Ok(task) => {
                let worktree = task.working_tree();
                if let Err(e) = crate::github::pr_checkout(&worktree, owner, repo_name, pr.number) {
                    result.errors.push(format!("{}: {}", task_key, e));
                    continue;
                }
                let key = ProjectKey::task(home.as_str(), &branch);
                crate::kv::set_value_sync(&key, "task_type", "review");
                crate::kv::set_value_sync(&key, "review_pr_url", &pr.url);
                crate::kv::set_value_sync(&key, "review_pr_title", &pr.title);
                if already_exists {
                    result.skipped.push(format!("{} (updated)", task_key));
                } else {
                    result.created.push(task_key);
                }
            }
            Err(e) => {
                result.errors.push(format!("{}: {}", task_key, e));
            }
        }
    }

    if !dry_run && (!result.created.is_empty() || !result.skipped.is_empty()) {
        projects::refresh_cache();
    }

    Ok(result)
}

#[derive(serde::Serialize)]
pub struct GithubTaskResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn create_github_ref_task(
    r: &crate::github::GithubRef,
    home_project: Option<&str>,
    dry_run: bool,
) -> Result<GithubTaskResult, String> {
    use crate::github::GithubRefKind;

    let kind = crate::github::resolve_github_ref_kind(r)?;
    let nwo = format!("{}/{}", r.owner, r.repo);

    let home = if let Some(h) = home_project {
        h.to_string()
    } else {
        let repo_map = build_github_repo_map();
        repo_map
            .get(&nwo)
            .map(|n| n.to_string())
            .ok_or_else(|| {
                format!(
                    "No local project for {}. Use --home-project to specify.",
                    nwo
                )
            })?
    };

    let branch = match kind {
        GithubRefKind::Pr => {
            crate::github::get_pr_branch(&r.owner, &r.repo, r.number).ok_or_else(|| {
                format!(
                    "Failed to get branch for PR #{} in {}",
                    r.number, nwo
                )
            })?
        }
        GithubRefKind::Issue => {
            let issue = crate::github::get_issue(&r.owner, &r.repo, r.number)?;
            let slug = crate::util::to_kebab_case(&issue.title);
            format!("{}-{}", r.number, slug)
        }
    };

    projects::refresh_tasks();
    let task_key_str = format!("{}:{}", home, branch);
    let existing = {
        let projects = projects::lock();
        projects
            .all()
            .iter()
            .any(|p| p.is_task() && p.store_key().to_string() == task_key_str)
    };

    if kind == GithubRefKind::Issue && existing {
        return Ok(GithubTaskResult {
            created: None,
            skipped: Some(format!("{} already exists", task_key_str)),
            error: None,
        });
    }

    if dry_run {
        let label = if existing { "update" } else { "create" };
        return Ok(GithubTaskResult {
            created: Some(format!("{} (dry run: {})", task_key_str, label)),
            skipped: None,
            error: None,
        });
    }

    let task = create_task(&home, &branch)?;
    let key = ProjectKey::task(&home, &branch);

    match kind {
        GithubRefKind::Pr => {
            crate::github::pr_checkout(&task.working_tree(), &r.owner, &r.repo, r.number)?;
            crate::kv::set_value_sync(&key, "task_type", "review");
            crate::kv::set_value_sync(
                &key,
                "review_pr_url",
                &format!("https://github.com/{}/pull/{}", nwo, r.number),
            );
        }
        GithubRefKind::Issue => {
            crate::kv::set_value_sync(&key, "task_type", "issue");
            crate::kv::set_value_sync(
                &key,
                "github_issue_url",
                &format!("https://github.com/{}/issues/{}", nwo, r.number),
            );
            crate::kv::set_value_sync(&key, "github_issue_number", &r.number.to_string());
        }
    }
    projects::refresh_cache();

    let label = if existing { "updated" } else { "created" };
    Ok(GithubTaskResult {
        created: Some(format!("{} ({})", task_key_str, label)),
        skipped: None,
        error: None,
    })
}

fn build_github_repo_map() -> std::collections::HashMap<String, config::CanonicalName> {
    let mut map = std::collections::HashMap::new();
    for (name, path) in config::available_projects() {
        if let Some(github_repo) = git::github_repo_from_remote(&path) {
            map.insert(github_repo, name);
        }
    }
    map
}

fn resolve_project_path(project_name: &str) -> Result<PathBuf, String> {
    config::resolve_project_name(project_name)
        .map(|(_, path)| path)
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
