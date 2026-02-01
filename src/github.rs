use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
pub struct PrStatus {
    pub number: u64,
    pub state: String,
    #[serde(rename = "isDraft")]
    pub is_draft: bool,
    pub url: String,
    #[serde(default, skip_deserializing)]
    pub comments: Vec<CommentCount>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct CommentCount {
    pub author: String,
    pub count: usize,
}

impl PrStatus {
    pub fn display(&self) -> String {
        let state = if self.is_draft {
            "draft"
        } else {
            match self.state.as_str() {
                "OPEN" => "open",
                "MERGED" => "merged",
                "CLOSED" => "closed",
                _ => &self.state,
            }
        };
        format!("#{} ({})", self.number, state)
    }

    pub fn comments_display(&self) -> Option<String> {
        if self.comments.is_empty() {
            return None;
        }
        let parts: Vec<String> = self
            .comments
            .iter()
            .map(|c| format!("{}:{}", c.author, c.count))
            .collect();
        Some(parts.join(" "))
    }
}

pub fn get_pr_status(project_path: &Path) -> Option<PrStatus> {
    let output = Command::new("gh")
        .args(["pr", "view", "--json", "number,state,isDraft,url"])
        .current_dir(project_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let mut pr: PrStatus = serde_json::from_slice(&output.stdout).ok()?;
    pr.comments = get_pr_comments(project_path, pr.number);
    Some(pr)
}

fn get_pr_comments(project_path: &Path, pr_number: u64) -> Vec<CommentCount> {
    let output = Command::new("gh")
        .args([
            "api",
            &format!("repos/{{owner}}/{{repo}}/pulls/{}/comments", pr_number),
            "--jq",
            ".[].user.login",
        ])
        .current_dir(project_path)
        .output()
        .ok();

    let review_comments: Vec<String> = output
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().map(String::from).collect())
        .unwrap_or_default();

    let output = Command::new("gh")
        .args([
            "api",
            &format!("repos/{{owner}}/{{repo}}/issues/{}/comments", pr_number),
            "--jq",
            ".[].user.login",
        ])
        .current_dir(project_path)
        .output()
        .ok();

    let issue_comments: Vec<String> = output
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().map(String::from).collect())
        .unwrap_or_default();

    let mut counts: HashMap<String, usize> = HashMap::new();
    for author in review_comments.into_iter().chain(issue_comments) {
        *counts.entry(author).or_default() += 1;
    }

    let mut result: Vec<CommentCount> = counts
        .into_iter()
        .map(|(author, count)| CommentCount { author, count })
        .collect();
    result.sort_by(|a, b| b.count.cmp(&a.count).then(a.author.cmp(&b.author)));
    result
}

/// Get the head branch name for a PR given owner/repo/number
pub fn get_pr_branch(owner: &str, repo: &str, pr_number: u64) -> Option<String> {
    let output = Command::new("gh")
        .args([
            "api",
            &format!("repos/{}/{}/pulls/{}", owner, repo, pr_number),
            "--jq",
            ".head.ref",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8(output.stdout).ok()?;
    let branch = branch.trim();
    if branch.is_empty() {
        None
    } else {
        Some(branch.to_string())
    }
}

use crate::project::Project;

/// Get the PR number for a project, checking cached value first
pub fn get_open_pr_number(project: &Project) -> Option<u64> {
    if let Some(pr) = project.cached.github_pr {
        return Some(pr);
    }
    fetch_pr_number(&project.working_tree())
}

/// Get the repo name for a project, checking cached value first
pub fn get_repo_name(project: &Project) -> Option<String> {
    if let Some(ref repo) = project.cached.github_repo {
        return Some(repo.clone());
    }
    fetch_repo_name(&project.working_tree())
}

/// Refresh GitHub info by fetching from gh CLI
pub fn refresh_github_info(project: &mut Project) {
    project.cached.github_pr = fetch_pr_number(&project.working_tree());
    project.cached.github_repo = fetch_repo_name(&project.working_tree());
}

fn fetch_pr_number(project_path: &Path) -> Option<u64> {
    let output = Command::new("gh")
        .args(["pr", "view", "--json", "number", "--jq", ".number"])
        .current_dir(project_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

fn fetch_repo_name(project_path: &Path) -> Option<String> {
    let output = Command::new("gh")
        .args([
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "--jq",
            ".nameWithOwner",
        ])
        .current_dir(project_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let name = String::from_utf8(output.stdout).ok()?;
    let name = name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}
