use std::env;
use std::io::{self, IsTerminal, Write};

use serde::Deserialize;

#[derive(Deserialize)]
struct SearchResponse {
    issues: Vec<Issue>,
}

#[derive(Deserialize, Clone)]
struct Issue {
    key: String,
    fields: Fields,
}

#[derive(Deserialize, Clone)]
struct Fields {
    summary: String,
    status: Status,
}

#[derive(Deserialize, Clone)]
struct Status {
    name: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct IssueStatus {
    pub key: String,
    pub summary: String,
    pub status: String,
}

impl IssueStatus {
    pub fn status_emoji(&self) -> &'static str {
        status_emoji(&self.status)
    }
}

pub fn status_emoji(status: &str) -> &'static str {
    match status.to_lowercase().as_str() {
        "done" | "closed" | "resolved" => "✅",
        "in progress" | "in development" => "🔵",
        "in review" | "code review" | "review" => "🟢",
        "blocked" => "🚫",
        _ => "⚫",
    }
}

fn auth_header() -> Result<String, String> {
    let email = env::var("JIRA_EMAIL").map_err(|_| "JIRA_EMAIL not set")?;
    let token = env::var("JIRA_TOKEN").map_err(|_| "JIRA_TOKEN not set")?;
    let credentials = format!("{}:{}", email, token);
    use base64::{engine::general_purpose::STANDARD, Engine};
    Ok(format!("Basic {}", STANDARD.encode(credentials)))
}

fn instance() -> Result<String, String> {
    env::var("JIRA_INSTANCE").map_err(|_| "JIRA_INSTANCE not set".to_string())
}

fn format_osc8_hyperlink(url: &str, text: &str) -> String {
    format!(
        "{osc}8;;{url}{st}{text}{osc}8;;{st}",
        url = url,
        text = text,
        osc = "\x1b]",
        st = "\x1b\\"
    )
}

fn format_key(key: &str, instance: &str) -> String {
    if io::stdout().is_terminal() {
        let url = format!("https://{}.atlassian.net/browse/{}", instance, key);
        format_osc8_hyperlink(&url, key)
    } else {
        key.to_string()
    }
}

pub fn get_issue(key: &str) -> Result<Option<IssueStatus>, String> {
    let instance = instance()?;
    let url = format!(
        "https://{}.atlassian.net/rest/api/3/issue/{}",
        instance, key
    );

    let response = ureq::get(&url)
        .query("fields", "summary,status")
        .set("Authorization", &auth_header()?)
        .set("Content-Type", "application/json")
        .call();

    match response {
        Ok(resp) => {
            let issue: Issue = resp
                .into_json()
                .map_err(|e| format!("Failed to parse JIRA response: {}", e))?;
            Ok(Some(IssueStatus {
                key: issue.key,
                summary: issue.fields.summary,
                status: issue.fields.status.name,
            }))
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(e) => Err(format!("JIRA request failed: {}", e)),
    }
}

pub fn get_sprint_issues() -> Result<Vec<IssueStatus>, String> {
    let instance = instance()?;
    let url = format!("https://{}.atlassian.net/rest/api/3/search/jql", instance);
    let jql = "assignee=currentUser() AND sprint in openSprints() ORDER BY key";

    let response: SearchResponse = ureq::get(&url)
        .query("jql", jql)
        .query("fields", "key,summary,status")
        .set("Authorization", &auth_header()?)
        .set("Content-Type", "application/json")
        .call()
        .map_err(|e| format!("JIRA request failed: {}", e))?
        .into_json()
        .map_err(|e| format!("Failed to parse JIRA response: {}", e))?;

    Ok(response
        .issues
        .into_iter()
        .map(|i| IssueStatus {
            key: i.key,
            summary: i.fields.summary,
            status: i.fields.status.name,
        })
        .collect())
}

pub fn print_sprint_issues() -> Result<(), String> {
    use std::thread;

    let instance = instance()?;
    let issues = get_sprint_issues()?;
    let repo = env::var("GITHUB_REPO").ok();

    let pr_statuses: Vec<_> = if let Some(ref repo) = repo {
        issues
            .iter()
            .map(|issue| {
                let repo = repo.clone();
                let branch = issue.key.clone();
                thread::spawn(move || crate::github::get_pr_for_branch(&repo, &branch))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().ok().flatten())
            .collect()
    } else {
        vec![None; issues.len()]
    };

    let mut stdout = io::stdout().lock();
    for (issue, pr) in issues.iter().zip(pr_statuses.iter()) {
        let pr_str = match pr {
            Some(pr) => format!("PR: {}", pr.display()),
            None => "PR: none".to_string(),
        };
        writeln!(
            stdout,
            "{} {} {}  {}",
            issue.status_emoji(),
            format_key(&issue.key, &instance),
            issue.summary,
            pr_str
        )
        .map_err(|e| format!("Write failed: {}", e))?;
    }
    Ok(())
}
