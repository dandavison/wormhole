use std::env;
use std::io::{self, IsTerminal, Write};

use serde::Deserialize;

#[derive(Deserialize)]
struct SearchResponse {
    issues: Vec<Issue>,
}

#[derive(Deserialize)]
struct Issue {
    key: String,
    fields: Fields,
}

#[derive(Deserialize)]
struct Fields {
    summary: String,
    status: Status,
}

#[derive(Deserialize)]
struct Status {
    name: String,
}

fn status_emoji(status: &str) -> &'static str {
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

pub fn print_sprint_issues() -> Result<(), String> {
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

    let mut stdout = io::stdout().lock();
    for issue in response.issues {
        writeln!(
            stdout,
            "{} {} {}",
            status_emoji(&issue.fields.status.name),
            format_key(&issue.key, &instance),
            issue.fields.summary
        )
        .map_err(|e| format!("Write failed: {}", e))?;
    }
    Ok(())
}
