use crate::jira;

use super::project::{
    render_issue_status, render_project_item, render_task_status, status_sort_order,
};
use super::util::Client;

pub(super) fn sprint_list(client: &Client, output: &str) -> Result<(), String> {
    use std::collections::HashSet;

    // Fetch sprint issues (client-side I/O)
    let sprint_keys: HashSet<String> = jira::get_sprint_issues()
        .map(|issues| issues.into_iter().map(|i| i.key).collect())
        .unwrap_or_default();

    // Get project list from server (in-memory, includes cached JIRA/PR)
    let response = client.get("/project/list")?;
    let json: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;

    // Filter to tasks with jira_key in sprint
    let mut sprint_tasks: Vec<&serde_json::Value> = json
        .get("current")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|item| {
                    item.get("kv")
                        .and_then(|kv| kv.get("jira_key"))
                        .and_then(|k| k.as_str())
                        .is_some_and(|k| sprint_keys.contains(k))
                })
                .collect()
        })
        .unwrap_or_default();
    sprint_tasks.sort_by_key(|item| {
        let status = item
            .get("jira")
            .and_then(|j| j.get("status"))
            .and_then(|s| s.as_str());
        status_sort_order(status)
    });

    if output == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&sprint_tasks).map_err(|e| e.to_string())?
        );
    } else {
        for item in sprint_tasks {
            println!("{}", render_project_item(item));
        }
    }
    Ok(())
}

/// Render a project item from /project/list response

pub(super) fn sprint_show(output: &str) -> Result<(), String> {
    use crate::status::TaskStatus;
    use std::thread;

    let issues = jira::get_sprint_issues()?;

    // Fetch status for each issue concurrently
    let statuses: Vec<_> = issues
        .iter()
        .map(|issue| {
            let key = issue.key.clone();
            let client_url = format!("http://127.0.0.1:{}", crate::config::wormhole_port());
            thread::spawn(move || {
                ureq::get(&format!("{}/project/show/{}", client_url, key))
                    .call()
                    .ok()
                    .and_then(|r| r.into_string().ok())
                    .and_then(|s| serde_json::from_str::<TaskStatus>(&s).ok())
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().ok().flatten())
        .collect();

    #[derive(serde::Serialize)]
    #[serde(tag = "type")]
    enum SprintShowItem {
        #[serde(rename = "task")]
        Task(TaskStatus),
        #[serde(rename = "issue")]
        Issue(crate::jira::IssueStatus),
    }

    let items: Vec<SprintShowItem> = issues
        .into_iter()
        .zip(statuses)
        .map(|(issue, status)| match status {
            Some(task) => SprintShowItem::Task(task),
            None => SprintShowItem::Issue(issue),
        })
        .collect();

    if output == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&items).map_err(|e| e.to_string())?
        );
    } else {
        for item in &items {
            match item {
                SprintShowItem::Task(task) => println!("{}\n\n", render_task_status(task)),
                SprintShowItem::Issue(issue) => {
                    println!("{}\n  (no wormhole task)\n\n", render_issue_status(issue))
                }
            }
        }
    }
    Ok(())
}
