use serde::{Deserialize, Serialize};

use crate::github::PrStatus;
use crate::jira::IssueStatus;
use crate::project::{Project, ProjectKey};
use crate::projects;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStatus {
    pub name: String,
    pub path: std::path::PathBuf,
    pub branch: Option<String>,
    pub jira: Option<IssueStatus>,
    pub pr: Option<PrStatus>,
    pub plan_exists: bool,
    pub plan_url: Option<String>,
    pub aux_repos: Option<String>,
}

pub fn get_status(project: &Project) -> TaskStatus {
    let name = project.repo_name.clone();
    let branch = project.branch.clone();
    let path = project
        .worktree_path()
        .unwrap_or_else(|| project.repo_path.clone());
    let kv = project.kv.clone();

    let plan_exists = path.join(".task/plan.md").exists();
    let plan_url = if plan_exists {
        crate::git::github_file_url(&path, ".task/plan.md")
    } else {
        None
    };
    let aux_repos = kv.get("aux-repos").cloned();

    TaskStatus {
        name,
        path,
        branch,
        jira: project.cached_jira.clone(),
        pr: project.cached_pr.clone(),
        plan_exists,
        plan_url,
        aux_repos,
    }
}

fn ensure_cache() {
    if projects::cache_needs_refresh() {
        projects::refresh_cache();
    }
}

pub fn get_status_by_name(name: &str) -> Option<TaskStatus> {
    ensure_cache();
    let projects = projects::lock();
    let key = ProjectKey::parse(name);
    let project = projects.by_key(&key).or_else(|| {
        let path = std::path::Path::new(name);
        crate::task::task_by_path(path).or_else(|| projects.by_path(path))
    })?;
    Some(get_status(&project))
}

pub fn get_current_status() -> Option<TaskStatus> {
    ensure_cache();
    let projects = projects::lock();
    let project = projects.current()?;
    Some(get_status(&project))
}
