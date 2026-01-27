use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::github;
use crate::task;

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
}

impl DescribeResponse {
    fn empty() -> Self {
        Self {
            name: None,
            kind: None,
            home_project: None,
            pr_branch: None,
        }
    }
}

pub fn describe(req: &DescribeRequest) -> DescribeResponse {
    if let Some(url) = &req.url {
        if let Some(gh) = parse_github_url(url) {
            return describe_github(&gh);
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
    let pr_branch = gh
        .pr
        .and_then(|pr| github::get_pr_branch(&gh.owner, &gh.repo, pr));

    // Try to find a task with this PR
    if let Some(pr_num) = gh.pr {
        if let Some((task_name, home)) = find_task_by_pr(&gh.owner, &gh.repo, pr_num) {
            return DescribeResponse {
                name: Some(task_name),
                kind: Some("task".to_string()),
                home_project: Some(home),
                pr_branch,
            };
        }
    }

    // Fall back to repo name as project
    DescribeResponse {
        name: Some(gh.repo.clone()),
        kind: Some("project".to_string()),
        home_project: None,
        pr_branch,
    }
}

fn find_task_by_pr(owner: &str, repo: &str, pr_number: u64) -> Option<(String, String)> {
    let tasks = task::tasks();
    for (name, project) in &tasks {
        if let Some(task_pr) = github::get_open_pr_number(&project.path) {
            if task_pr == pr_number {
                // Verify it's the same repo
                if let Some(task_repo) = github::get_repo_name(&project.path) {
                    if task_repo == format!("{}/{}", owner, repo) {
                        return Some((
                            name.clone(),
                            project.home_project.clone().unwrap_or_default(),
                        ));
                    }
                }
            }
        }
    }
    None
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
}
