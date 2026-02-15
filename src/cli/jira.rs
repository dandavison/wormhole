use crate::handlers::jira::{SprintShowItem, SprintShowResult};

use super::project::{render_issue_status, render_project_item, render_task_status};
use super::util::Client;

pub(super) fn sprint_list(client: &Client, output: &str) -> Result<(), String> {
    let response = client.get("/jira/sprint/list")?;
    if output == "json" {
        println!("{}", response);
    } else {
        let result: crate::handlers::jira::SprintListResult =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        for item in &result.tasks {
            println!("{}", render_project_item(item));
        }
    }
    Ok(())
}

pub(super) fn sprint_show(client: &Client, output: &str) -> Result<(), String> {
    let response = client.get("/jira/sprint/show")?;
    if output == "json" {
        println!("{}", response);
    } else {
        let result: SprintShowResult =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        for item in &result.items {
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
