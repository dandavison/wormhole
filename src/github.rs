use regex::Regex;
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

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct ReviewRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub repository: SearchRepository,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct SearchRepository {
    pub name: String,
    #[serde(rename = "nameWithOwner")]
    pub name_with_owner: String,
}

pub fn search_review_requests() -> Result<Vec<ReviewRequest>, String> {
    let output = Command::new("gh")
        .args([
            "search",
            "prs",
            "user-review-requested:@me",
            "--state=open",
            "--limit",
            "100",
            "--json",
            "number,title,url,repository",
        ])
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh search prs failed: {}", stderr.trim()));
    }

    serde_json::from_slice(&output.stdout).map_err(|e| format!("Failed to parse gh output: {}", e))
}

pub fn pr_checkout(
    worktree_path: &Path,
    owner: &str,
    repo: &str,
    pr_number: u64,
) -> Result<(), String> {
    let output = Command::new("gh")
        .args([
            "pr",
            "checkout",
            &pr_number.to_string(),
            "--force",
            "--repo",
            &format!("{}/{}", owner, repo),
        ])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| format!("gh pr checkout failed: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh pr checkout failed: {}", stderr.trim()));
    }
    Ok(())
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct GithubIssue {
    pub number: u64,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GithubRefKind {
    Pr,
    Issue,
}

/// A parsed GitHub reference: owner, repo, number, and optionally whether it's
/// a PR or issue (known from URL path, unknown from short `owner/repo#N` form).
#[derive(Debug, Clone)]
pub struct GithubRef {
    pub owner: String,
    pub repo: String,
    pub number: u64,
    pub kind: Option<GithubRefKind>,
}

/// Parse a GitHub reference: PR URL, issue URL, or `owner/repo#123`.
/// For URLs the kind is unambiguous; for the short form it is `None`.
pub fn parse_github_ref(input: &str) -> Option<GithubRef> {
    // PR URL: https://github.com/owner/repo/pull/123
    let pr_re = Regex::new(r"github\.com/([^/]+)/([^/]+)/pull/(\d+)").ok()?;
    if let Some(caps) = pr_re.captures(input) {
        return Some(GithubRef {
            owner: caps[1].to_string(),
            repo: caps[2].to_string(),
            number: caps[3].parse().ok()?,
            kind: Some(GithubRefKind::Pr),
        });
    }
    // Issue URL: https://github.com/owner/repo/issues/123
    let issue_re = Regex::new(r"github\.com/([^/]+)/([^/]+)/issues/(\d+)").ok()?;
    if let Some(caps) = issue_re.captures(input) {
        return Some(GithubRef {
            owner: caps[1].to_string(),
            repo: caps[2].to_string(),
            number: caps[3].parse().ok()?,
            kind: Some(GithubRefKind::Issue),
        });
    }
    // Short ref: owner/repo#123 (ambiguous)
    let ref_re = Regex::new(r"^([^/]+)/([^#]+)#(\d+)$").ok()?;
    if let Some(caps) = ref_re.captures(input) {
        return Some(GithubRef {
            owner: caps[1].to_string(),
            repo: caps[2].to_string(),
            number: caps[3].parse().ok()?,
            kind: None,
        });
    }
    None
}

/// Resolve the kind of a GitHub ref by querying the API.
/// Tries PR first (more common), falls back to issue.
pub fn resolve_github_ref_kind(r: &GithubRef) -> Result<GithubRefKind, String> {
    if let Some(kind) = r.kind {
        return Ok(kind);
    }
    if get_pr_branch(&r.owner, &r.repo, r.number).is_some() {
        return Ok(GithubRefKind::Pr);
    }
    if get_issue(&r.owner, &r.repo, r.number).is_ok() {
        return Ok(GithubRefKind::Issue);
    }
    Err(format!(
        "#{} is neither a PR nor an issue in {}/{}",
        r.number, r.owner, r.repo
    ))
}


pub fn get_issue(owner: &str, repo: &str, number: u64) -> Result<GithubIssue, String> {
    let output = Command::new("gh")
        .args([
            "issue",
            "view",
            &number.to_string(),
            "--repo",
            &format!("{}/{}", owner, repo),
            "--json",
            "number,title,url,body",
        ])
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh issue view failed: {}", stderr.trim()));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("Failed to parse gh output: {}", e))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_ref_pr_url() {
        let r = parse_github_ref("https://github.com/temporalio/temporal/pull/9515").unwrap();
        assert_eq!(r.owner, "temporalio");
        assert_eq!(r.repo, "temporal");
        assert_eq!(r.number, 9515);
        assert_eq!(r.kind, Some(GithubRefKind::Pr));
    }

    #[test]
    fn parse_github_ref_issue_url() {
        let r = parse_github_ref("https://github.com/temporalio/temporal/issues/42").unwrap();
        assert_eq!(r.owner, "temporalio");
        assert_eq!(r.repo, "temporal");
        assert_eq!(r.number, 42);
        assert_eq!(r.kind, Some(GithubRefKind::Issue));
    }

    #[test]
    fn parse_github_ref_short_form_is_ambiguous() {
        let r = parse_github_ref("temporalio/temporal#123").unwrap();
        assert_eq!(r.owner, "temporalio");
        assert_eq!(r.repo, "temporal");
        assert_eq!(r.number, 123);
        assert_eq!(r.kind, None);
    }

    #[test]
    fn parse_github_ref_invalid() {
        assert!(parse_github_ref("not-a-ref").is_none());
        assert!(parse_github_ref("ACT-123").is_none());
        assert!(parse_github_ref("just/repo").is_none());
    }

    #[test]
    fn parse_github_ref_pr_url_not_issue() {
        let r = parse_github_ref("https://github.com/temporalio/temporal/pull/42").unwrap();
        assert_eq!(r.kind, Some(GithubRefKind::Pr));
    }

    #[test]
    fn parse_github_ref_issue_url_not_pr() {
        let r = parse_github_ref("https://github.com/temporalio/temporal/issues/42").unwrap();
        assert_eq!(r.kind, Some(GithubRefKind::Issue));
    }
}
