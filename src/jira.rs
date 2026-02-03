use std::env;

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
    assignee: Option<Assignee>,
    #[serde(default)]
    sprint: SprintField,
}

#[derive(Deserialize, Clone, Default)]
#[serde(untagged)]
enum SprintField {
    #[default]
    None,
    Single(Sprint),
    Array(Vec<Sprint>),
}

impl SprintField {
    fn active_sprint(&self) -> Option<&Sprint> {
        match self {
            SprintField::None => None,
            SprintField::Single(s) => Some(s),
            SprintField::Array(arr) => arr.iter().find(|s| s.state == "active").or(arr.last()),
        }
    }
}

#[derive(Deserialize, Clone)]
struct Sprint {
    name: String,
    #[serde(default)]
    state: String,
}

#[derive(Deserialize, Clone)]
struct Assignee {
    #[serde(rename = "emailAddress")]
    email_address: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Deserialize, Clone)]
struct Status {
    name: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IssueStatus {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub assignee: Option<String>,
    pub assignee_email: Option<String>,
    #[serde(default)]
    pub sprint: Option<String>,
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

pub fn get_issue(key: &str) -> Result<Option<IssueStatus>, String> {
    let instance = instance()?;
    // Use Agile API to get sprint data (standard API doesn't return it)
    let url = format!(
        "https://{}.atlassian.net/rest/agile/1.0/issue/{}",
        instance, key
    );

    let response = ureq::get(&url)
        .query("fields", "summary,status,assignee,sprint")
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
                assignee: issue
                    .fields
                    .assignee
                    .as_ref()
                    .and_then(|a| a.display_name.clone()),
                assignee_email: issue
                    .fields
                    .assignee
                    .as_ref()
                    .and_then(|a| a.email_address.clone()),
                sprint: issue.fields.sprint.active_sprint().map(|s| s.name.clone()),
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
        .query("fields", "key,summary,status,assignee,sprint")
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
            assignee: i
                .fields
                .assignee
                .as_ref()
                .and_then(|a| a.display_name.clone()),
            assignee_email: i
                .fields
                .assignee
                .as_ref()
                .and_then(|a| a.email_address.clone()),
            sprint: i.fields.sprint.active_sprint().map(|s| s.name.clone()),
        })
        .collect())
}
