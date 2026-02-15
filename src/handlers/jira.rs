use hyper::{Body, Response, StatusCode};
use serde::{Deserialize, Serialize};

use crate::jira::{self, IssueStatus};
use crate::status::{self, TaskStatus};

// --- sprint list ---

#[derive(Serialize, Deserialize)]
pub struct SprintListResult {
    pub tasks: Vec<serde_json::Value>,
}

pub fn sprint_list() -> Response<Body> {
    use std::collections::HashSet;

    let sprint_keys: HashSet<String> = match jira::get_sprint_issues() {
        Ok(issues) => issues.into_iter().map(|i| i.key).collect(),
        Err(e) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(e))
                .unwrap()
        }
    };

    let open_projects = crate::projects::lock().open();

    let mut tasks: Vec<serde_json::Value> = open_projects
        .into_iter()
        .filter_map(|project| {
            let jira_key = project.kv.get("jira_key")?;
            if !sprint_keys.contains(jira_key) {
                return None;
            }
            let mut obj = serde_json::json!({
                "project_key": project.store_key().to_string()
            });
            let path = project
                .worktree_path()
                .unwrap_or_else(|| project.repo_path.clone());
            obj["path"] = serde_json::json!(path);
            if !project.kv.is_empty() {
                obj["kv"] = serde_json::json!(project.kv);
            }
            if let Some(ref j) = project.cached.jira {
                obj["jira"] = serde_json::json!(j);
            }
            if let Some(ref pr) = project.cached.pr {
                obj["pr"] = serde_json::json!(pr);
            }
            Some(obj)
        })
        .collect();

    tasks.sort_by_key(|item| {
        let status = item
            .get("jira")
            .and_then(|j| j.get("status"))
            .and_then(|s| s.as_str());
        status_sort_order(status)
    });

    let result = SprintListResult { tasks };
    json_response(&result)
}

// --- sprint show ---

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SprintShowItem {
    #[serde(rename = "task")]
    Task(TaskStatus),
    #[serde(rename = "issue")]
    Issue(IssueStatus),
}

#[derive(Serialize, Deserialize)]
pub struct SprintShowResult {
    pub items: Vec<SprintShowItem>,
}

pub fn sprint_show() -> Response<Body> {
    let issues = match jira::get_sprint_issues() {
        Ok(issues) => issues,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(e))
                .unwrap()
        }
    };

    let items: Vec<SprintShowItem> = issues
        .into_iter()
        .map(|issue| match status::get_status_by_name(&issue.key) {
            Some(task) => SprintShowItem::Task(task),
            None => SprintShowItem::Issue(issue),
        })
        .collect();

    let result = SprintShowResult { items };
    json_response(&result)
}

fn status_sort_order(status: Option<&str>) -> u8 {
    match status.map(|s| s.to_lowercase()).as_deref() {
        Some("done") | Some("closed") | Some("resolved") => 0,
        Some("in review") => 1,
        Some("in progress") => 2,
        Some("to do") => 3,
        _ => 4,
    }
}

fn json_response<T: Serialize>(value: &T) -> Response<Body> {
    match serde_json::to_string(value) {
        Ok(json) => Response::builder()
            .header("Content-Type", "application/json")
            .body(Body::from(json))
            .unwrap(),
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(format!("Failed to serialize: {}", e)))
            .unwrap(),
    }
}
