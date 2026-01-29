use std::thread;

use serde::{Deserialize, Serialize};

use crate::github::{self, PrStatus};
use crate::jira::{self, IssueStatus};
use crate::project::{Project, StoreKey};
use crate::projects;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum SprintShowItem {
    #[serde(rename = "task")]
    Task(TaskStatus),
    #[serde(rename = "issue")]
    Issue(IssueStatus),
}

impl SprintShowItem {
    pub fn render_terminal(&self) -> String {
        match self {
            SprintShowItem::Task(task) => task.render_terminal(),
            SprintShowItem::Issue(issue) => {
                format!("{}\n  (no wormhole task)", issue.render_terminal())
            }
        }
    }
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

impl TaskStatus {
    pub fn render_terminal(&self) -> String {
        let jira_instance = std::env::var("JIRA_INSTANCE").ok();

        let name_linked = if let Some(ref instance) = jira_instance {
            let url = format!("https://{}.atlassian.net/browse/{}", instance, self.name);
            crate::format_osc8_hyperlink(&url, &self.name)
        } else {
            self.name.clone()
        };

        let title = if let Some(ref jira) = self.jira {
            format!("{}: {}", name_linked, jira.summary)
        } else {
            name_linked.clone()
        };
        let title_len = if let Some(ref jira) = self.jira {
            self.name.len() + 2 + jira.summary.len()
        } else {
            self.name.len()
        };

        let mut lines = vec![title, "─".repeat(title_len)];

        if let Some(ref branch) = self.branch {
            lines.push(format!("Branch:    {}", branch));
        }

        if let Some(ref jira) = self.jira {
            lines.push(format!(
                "JIRA:      {} {}",
                jira.status_emoji(),
                jira.status
            ));
        } else if self.branch.is_some() {
            lines.push("JIRA:      ✗".to_string());
        }

        if let Some(ref pr) = self.pr {
            let pr_linked = crate::format_osc8_hyperlink(&pr.url, &pr.display());
            let comments = pr
                .comments_display()
                .map(|c| format!(" [{}]", c))
                .unwrap_or_default();
            lines.push(format!("PR:        {}{}", pr_linked, comments));
        } else {
            lines.push("PR:        ✗".to_string());
        }

        if let Some(ref url) = self.plan_url {
            let plan_linked = crate::format_osc8_hyperlink(url, "✓ plan.md");
            lines.push(format!("Plan:      {}", plan_linked));
        } else {
            lines.push("Plan:      ✗".to_string());
        }

        if let Some(ref repos) = self.aux_repos {
            lines.push(format!("Aux repos: {}", repos));
        } else {
            lines.push("Aux repos: ✗".to_string());
        }

        lines.join("\n")
    }
}
