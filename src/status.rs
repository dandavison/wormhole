use std::thread;

use serde::{Deserialize, Serialize};

use crate::github::{self, PrStatus};
use crate::jira::{self, IssueStatus};
use crate::project::Project;
use crate::projects;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum SprintShowItem {
    #[serde(rename = "task")]
    Task(TaskStatus),
    #[serde(rename = "issue")]
    Issue(IssueStatus),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStatus {
    pub name: String,
    pub home_project: Option<String>,
    pub jira: Option<IssueStatus>,
    pub pr: Option<PrStatus>,
    pub plan_exists: bool,
    pub plan_url: Option<String>,
    pub aux_repos: Option<String>,
}

pub fn get_status(project: &Project) -> TaskStatus {
    let name = project.name.clone();
    let home_project = project.home_project.clone();
    let path = project.path.clone();
    let kv = project.kv.clone();

    let is_task = home_project.is_some();

    let jira_handle = if is_task {
        let key = name.clone();
        Some(thread::spawn(move || jira::get_issue(&key).ok().flatten()))
    } else {
        None
    };

    let pr_handle = {
        let path = path.clone();
        thread::spawn(move || github::get_pr_status(&path))
    };

    let plan_exists = path.join("plan.md").exists();
    let plan_url = if plan_exists {
        crate::git::github_file_url(&path, "plan.md")
    } else {
        None
    };
    let aux_repos = kv.get("aux-repos").cloned();

    let jira = jira_handle.and_then(|h| h.join().ok()).flatten();
    let pr = pr_handle.join().ok().flatten();

    TaskStatus {
        name,
        home_project,
        jira,
        pr,
        plan_exists,
        plan_url,
        aux_repos,
    }
}

pub fn get_status_by_name(name: &str) -> Option<TaskStatus> {
    let projects = projects::lock();
    let project = projects.by_name(name)?;
    Some(get_status(&project))
}

pub fn get_current_status() -> Option<TaskStatus> {
    let projects = projects::lock();
    let project = projects.current()?;
    Some(get_status(&project))
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

        let mut lines = vec![title, "─".repeat(title_len.min(60))];

        if let Some(ref home) = self.home_project {
            lines.push(format!("Home:      {}", home));
        }

        if let Some(ref jira) = self.jira {
            lines.push(format!("JIRA:      {} {}", jira.status_emoji(), jira.status));
        } else if self.home_project.is_some() {
            lines.push("JIRA:      ✗".to_string());
        }

        if let Some(ref pr) = self.pr {
            let pr_linked = crate::format_osc8_hyperlink(&pr.url, &pr.display());
            lines.push(format!("PR:        {}", pr_linked));
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
