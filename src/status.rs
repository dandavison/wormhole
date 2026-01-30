use std::thread;

use serde::{Deserialize, Serialize};

use crate::github::{self, PrStatus};
use crate::jira::{self, IssueStatus};
use crate::project::{Project, StoreKey};
use crate::projects;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum SprintShowItem {
    #[serde(rename = "task")]
    Task(TaskStatus),
    #[serde(rename = "issue")]
    Issue(IssueStatus),
}

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

    let is_task = project.is_task();

    let jira_handle = if is_task {
        // JIRA key is stored in kv if available
        kv.get("jira_key")
            .cloned()
            .map(|key| thread::spawn(move || jira::get_issue(&key).ok().flatten()))
    } else {
        None
    };

    let pr_handle = {
        let path = path.clone();
        thread::spawn(move || github::get_pr_status(&path))
    };

    let plan_exists = path.join(".task/plan.md").exists();
    let plan_url = if plan_exists {
        crate::git::github_file_url(&path, ".task/plan.md")
    } else {
        None
    };
    let aux_repos = kv.get("aux-repos").cloned();

    let jira = jira_handle.and_then(|h| h.join().ok()).flatten();
    let pr = pr_handle.join().ok().flatten();

    TaskStatus {
        name,
        path,
        branch,
        jira,
        pr,
        plan_exists,
        plan_url,
        aux_repos,
    }
}

pub fn get_status_by_name(name: &str) -> Option<TaskStatus> {
    let projects = projects::lock();
    let key = StoreKey::parse(name);
    let project = projects.by_key(&key).or_else(|| {
        let path = std::path::Path::new(name);
        crate::task::task_by_path(path).or_else(|| projects.by_path(path))
    })?;
    Some(get_status(&project))
}

pub fn get_current_status() -> Option<TaskStatus> {
    let projects = projects::lock();
    let project = projects.current()?;
    Some(get_status(&project))
}

pub fn get_sprint_status() -> Vec<SprintShowItem> {
    let issues = match jira::get_sprint_issues() {
        Ok(issues) => issues,
        Err(_) => return vec![],
    };
    let projects = projects::lock();
    issues
        .into_iter()
        .map(|issue| {
            let task = projects
                .all()
                .into_iter()
                .find(|p| p.kv.get("jira_key").is_some_and(|k| k == &issue.key));
            match task {
                Some(project) => SprintShowItem::Task(get_status(project)),
                None => SprintShowItem::Issue(issue),
            }
        })
        .collect()
}
