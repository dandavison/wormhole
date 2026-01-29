use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::thread;

use crate::github;
use crate::project::StoreKey;
use crate::projects;

#[derive(Debug, Deserialize)]
pub struct DescribeRequest {
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DescribeResponse {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub home_project: Option<String>,
    pub pr_branch: Option<String>,
    pub jira_url: Option<String>,
    pub jira_key: Option<String>,
    pub github_url: Option<String>,
    pub github_label: Option<String>,
}

impl DescribeResponse {
    fn empty() -> Self {
        Self {
            name: None,
            kind: None,
            home_project: None,
            pr_branch: None,
            jira_url: None,
            jira_key: None,
            github_url: None,
            github_label: None,
        }
    }
}

pub fn describe(req: &DescribeRequest) -> DescribeResponse {
    if let Some(url) = &req.url {
        if let Some(gh) = parse_github_url(url) {
            return describe_github(&gh);
        }
        if let Some(jira_key) = parse_jira_url(url) {
            return describe_jira(&jira_key);
        }
    }
    DescribeResponse::empty()
}

struct GitHubUrl {
    owner: String,
    repo: String,
    pr: Option<u64>,
}

fn parse_github_url(url: &str) -> Option<GitHubUrl> {
    let pr_re = Regex::new(r"github\.com/([^/]+)/([^/]+)/pull/(\d+)").ok()?;
    if let Some(caps) = pr_re.captures(url) {
        return Some(GitHubUrl {
            owner: caps[1].to_string(),
            repo: caps[2].to_string(),
            pr: caps[3].parse().ok(),
        });
    }

    let repo_re = Regex::new(r"github\.com/([^/]+)/([^/]+)").ok()?;
    if let Some(caps) = repo_re.captures(url) {
        let repo = caps[2].to_string();
        if !["settings", "notifications", "new", "login", "signup"].contains(&repo.as_str()) {
            return Some(GitHubUrl {
                owner: caps[1].to_string(),
                repo,
                pr: None,
            });
        }
    }

    None
}

fn describe_github(gh: &GitHubUrl) -> DescribeResponse {
    let (tx, rx) = mpsc::channel();

    // Fetch PR branch in parallel with task search
    let owner = gh.owner.clone();
    let repo = gh.repo.clone();
    let pr = gh.pr;
    thread::spawn(move || {
        let branch = pr.and_then(|n| github::get_pr_branch(&owner, &repo, n));
        let _ = tx.send(branch);
    });

    // Search tasks in parallel
    let task_match = gh
        .pr
        .and_then(|pr_num| find_task_by_pr(&gh.owner, &gh.repo, pr_num));

    let pr_branch = rx.recv().ok().flatten();

    match task_match {
        Some((store_key, home)) => DescribeResponse {
            name: Some(store_key.to_string()),
            kind: Some("task".to_string()),
            home_project: Some(home),
            pr_branch,
            jira_url: None,
            jira_key: None,
            github_url: None,
            github_label: None,
        },
        None => DescribeResponse {
            name: Some(gh.repo.clone()),
            kind: Some("project".to_string()),
            home_project: None,
            pr_branch,
            jira_url: None,
            jira_key: None,
            github_url: None,
            github_label: None,
        },
    }
}

fn parse_jira_url(url: &str) -> Option<String> {
    // Match URLs like https://temporalio.atlassian.net/browse/ACT-108
    let browse_re = Regex::new(r"atlassian\.net/browse/([A-Z]+-\d+)").ok()?;
    if let Some(caps) = browse_re.captures(url) {
        return Some(caps[1].to_string());
    }

    // Match board URLs with selectedIssue query param
    // e.g., .../boards/72?...&selectedIssue=ACT-108
    let selected_re = Regex::new(r"selectedIssue=([A-Z]+-\d+)").ok()?;
    if let Some(caps) = selected_re.captures(url) {
        return Some(caps[1].to_string());
    }

    None
}

fn describe_jira(jira_key: &str) -> DescribeResponse {
    // Find task by JIRA key stored in kv
    let tasks = projects::tasks();
    let project = tasks
        .values()
        .find(|p| p.kv.get("jira_key").is_some_and(|k| k == jira_key));

    if let Some(project) = project {
        let pr_number = github::get_open_pr_number(project);
        let repo_name = github::get_repo_name(project);

        let (github_url, github_label) = match (pr_number, repo_name) {
            (Some(pr), Some(repo)) => {
                let short_repo = repo.split('/').next_back().unwrap_or(&repo);
                (
                    Some(format!("https://github.com/{}/pull/{}", repo, pr)),
                    Some(format!("{}#{}", short_repo, pr)),
                )
            }
            _ => (None, None),
        };

        let jira_url = jira_url_for_key(jira_key);

        DescribeResponse {
            name: Some(jira_key.to_string()),
            kind: Some("task".to_string()),
            home_project: if project.is_task() {
                Some(project.repo_name.clone())
            } else {
                None
            },
            pr_branch: project.branch.clone(),
            jira_url,
            jira_key: Some(jira_key.to_string()),
            github_url,
            github_label,
        }
    } else {
        // No task found, but we know it's a valid JIRA key
        DescribeResponse {
            name: Some(jira_key.to_string()),
            kind: None,
            home_project: None,
            pr_branch: None,
            jira_url: jira_url_for_key(jira_key),
            jira_key: Some(jira_key.to_string()),
            github_url: None,
            github_label: None,
        }
    }
}

fn jira_url_for_key(key: &str) -> Option<String> {
    // Check if key looks like a JIRA key (e.g., "ACT-708", "PROJ-123")
    let jira_key_re = Regex::new(r"^[A-Z]+-\d+").ok()?;
    if !jira_key_re.is_match(key) {
        return None;
    }
    let instance = std::env::var("JIRA_INSTANCE").ok()?;
    Some(format!("https://{}.atlassian.net/browse/{}", instance, key))
}

fn find_task_by_pr(owner: &str, repo: &str, pr_number: u64) -> Option<(StoreKey, String)> {
    let expected_repo = format!("{}/{}", owner, repo);
    let tasks: Vec<(StoreKey, crate::project::Project)> = projects::tasks().into_iter().collect();

    tasks.par_iter().find_map_any(|(key, project)| {
        let task_pr = github::get_open_pr_number(project)?;
        if task_pr != pr_number {
            return None;
        }
        let task_repo = github::get_repo_name(project)?;
        if task_repo != expected_repo {
            return None;
        }
        Some((key.clone(), project.repo_name.clone()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_pr_url() {
        let url = "https://github.com/temporalio/temporal/pull/9146";
        let gh = parse_github_url(url).unwrap();
        assert_eq!(gh.owner, "temporalio");
        assert_eq!(gh.repo, "temporal");
        assert_eq!(gh.pr, Some(9146));
    }

    #[test]
    fn test_parse_github_pr_url_with_path() {
        let url = "https://github.com/temporalio/temporal/pull/9146/files";
        let gh = parse_github_url(url).unwrap();
        assert_eq!(gh.owner, "temporalio");
        assert_eq!(gh.repo, "temporal");
        assert_eq!(gh.pr, Some(9146));
    }

    #[test]
    fn test_parse_github_repo_url() {
        let url = "https://github.com/temporalio/temporal";
        let gh = parse_github_url(url).unwrap();
        assert_eq!(gh.owner, "temporalio");
        assert_eq!(gh.repo, "temporal");
        assert_eq!(gh.pr, None);
    }

    #[test]
    fn test_parse_github_repo_url_with_path() {
        let url = "https://github.com/temporalio/temporal/tree/main/src";
        let gh = parse_github_url(url).unwrap();
        assert_eq!(gh.owner, "temporalio");
        assert_eq!(gh.repo, "temporal");
        assert_eq!(gh.pr, None);
    }

    #[test]
    fn test_jira_url_for_valid_key() {
        std::env::set_var("JIRA_INSTANCE", "testinst");
        let url = jira_url_for_key("ACT-708");
        assert_eq!(
            url,
            Some("https://testinst.atlassian.net/browse/ACT-708".to_string())
        );
    }

    #[test]
    fn test_jira_url_for_invalid_key() {
        std::env::set_var("JIRA_INSTANCE", "testinst");
        let url = jira_url_for_key("not-a-jira-key");
        assert_eq!(url, None);
    }

    #[test]
    fn test_jira_url_for_repo_name() {
        std::env::set_var("JIRA_INSTANCE", "testinst");
        let url = jira_url_for_key("temporal");
        assert_eq!(url, None);
    }

    #[test]
    fn test_parse_jira_url() {
        let url = "https://temporalio.atlassian.net/browse/ACT-108";
        let key = parse_jira_url(url).unwrap();
        assert_eq!(key, "ACT-108");
    }

    #[test]
    fn test_parse_jira_url_with_query() {
        let url = "https://temporalio.atlassian.net/browse/ACT-108?focusedWorklogId=123";
        let key = parse_jira_url(url).unwrap();
        assert_eq!(key, "ACT-108");
    }

    #[test]
    fn test_parse_jira_url_invalid() {
        let url = "https://github.com/temporalio/temporal/pull/9146";
        assert!(parse_jira_url(url).is_none());
    }

    #[test]
    fn test_parse_jira_board_url() {
        let url = "https://temporalio.atlassian.net/jira/software/c/projects/ACT/boards/72?assignee=712020&selectedIssue=ACT-108";
        let key = parse_jira_url(url).unwrap();
        assert_eq!(key, "ACT-108");
    }
}
