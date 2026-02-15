use serde::Serialize;

use crate::jira;
use crate::project::ProjectKey;
use crate::pst::TerminalHyperlink;

#[derive(Serialize, serde::Deserialize)]
pub(super) struct ProjectDebug {
    index: usize,
    name: String,
    path: String,
    home_project: Option<String>,
}

impl ProjectDebug {
    pub(super) fn render_terminal(&self) -> String {
        let name_linked = ProjectKey::parse(&self.name).hyperlink();
        format!(
            "[{}] name: {}, path: {}",
            self.index, name_linked, self.path
        )
    }
}

#[derive(Serialize)]
pub(super) struct KvValue {
    pub(super) project: String,
    pub(super) key: String,
    pub(super) value: Option<String>,
}

impl KvValue {
    pub(super) fn render_terminal(&self) -> String {
        self.value.clone().unwrap_or_default()
    }
}

pub(super) fn status_sort_order(status: Option<&str>) -> u8 {
    match status.map(|s| s.to_lowercase()).as_deref() {
        Some("done") | Some("closed") | Some("resolved") => 0,
        Some("in review") => 1,
        Some("in progress") => 2,
        Some("to do") => 3,
        _ => 4,
    }
}

/// Render a project item from /project/list response
pub(super) fn render_project_item(item: &serde_json::Value) -> String {
    let project_key_str = item
        .get("project_key")
        .and_then(|k| k.as_str())
        .unwrap_or("");
    let task_display = ProjectKey::parse(project_key_str).hyperlink();

    let jira_instance = std::env::var("JIRA_INSTANCE").ok();
    let (jira_key, status) = item
        .get("jira")
        .map(|j| {
            (
                j.get("key").and_then(|k| k.as_str()).unwrap_or(""),
                j.get("status").and_then(|s| s.as_str()).unwrap_or(""),
            )
        })
        .unwrap_or(("", ""));

    let pr_display = item
        .get("pr")
        .map(|p| {
            let url = p.get("url").and_then(|u| u.as_str()).unwrap_or("");
            let number = p.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
            let is_draft = p.get("isDraft").and_then(|d| d.as_bool()).unwrap_or(false);
            let display = if is_draft {
                format!("#{} (draft)", number)
            } else {
                format!("#{}", number)
            };
            format!("  {}", crate::format_osc8_hyperlink(url, &display))
        })
        .unwrap_or_default();

    // No JIRA info - just show the task identifier
    if jira_key.is_empty() {
        return task_display;
    }

    let jira_display = if let Some(ref instance) = jira_instance {
        let url = format!("https://{}.atlassian.net/browse/{}", instance, jira_key);
        crate::format_osc8_hyperlink(&url, jira_key)
    } else {
        jira_key.to_string()
    };

    let indicator = jira::status_indicator(status);
    let pad = 40_usize.saturating_sub(project_key_str.len());

    format!(
        "{} {}{} {}{}",
        indicator,
        task_display,
        " ".repeat(pad),
        jira_display,
        pr_display
    )
}

pub(super) fn render_task_status(status: &crate::status::TaskStatus) -> String {
    let project_key = match &status.branch {
        Some(branch) => ProjectKey::task(&status.name, branch),
        None => ProjectKey::project(&status.name),
    };
    let name_linked = project_key.hyperlink();
    let name_display = project_key.to_string();

    let title = if let Some(ref jira) = status.jira {
        format!("{}: {}", name_linked, jira.summary)
    } else {
        name_linked.clone()
    };
    let title_len = if let Some(ref jira) = status.jira {
        name_display.len() + 2 + jira.summary.len()
    } else {
        name_display.len()
    };

    let mut lines = vec![title, "─".repeat(title_len)];

    if let Some(ref branch) = status.branch {
        lines.push(format!("Branch:    {}", branch));
    }

    if let Some(ref jira) = status.jira {
        lines.push(format!(
            "JIRA:      {} {}",
            crate::jira::status_indicator(&jira.status),
            jira.status
        ));
    } else if status.branch.is_some() {
        lines.push("JIRA:      ✗".to_string());
    }

    if let Some(ref pr) = status.pr {
        let pr_linked = crate::format_osc8_hyperlink(&pr.url, &pr.display());
        let comments = pr
            .comments_display()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();
        lines.push(format!("PR:        {}{}", pr_linked, comments));
    } else {
        lines.push("PR:        ✗".to_string());
    }

    if let Some(ref url) = status.claude_md_url {
        let linked = crate::format_osc8_hyperlink(url, "✓ CLAUDE.md");
        lines.push(format!("CLAUDE.md: {}", linked));
    } else {
        lines.push("CLAUDE.md: ✗".to_string());
    }

    if let Some(ref repos) = status.aux_repos {
        lines.push(format!("Aux repos: {}", repos));
    } else {
        lines.push("Aux repos: ✗".to_string());
    }

    lines.join("\n")
}

pub(super) fn render_issue_status(issue: &crate::jira::IssueStatus) -> String {
    let jira_instance = std::env::var("JIRA_INSTANCE").ok();
    let key_display = if let Some(ref instance) = jira_instance {
        let url = format!("https://{}.atlassian.net/browse/{}", instance, issue.key);
        crate::format_osc8_hyperlink(&url, &issue.key)
    } else {
        issue.key.clone()
    };
    format!(
        "{} {}: {}",
        crate::jira::status_indicator(&issue.status),
        key_display,
        issue.summary
    )
}

pub(super) fn for_each(
    client: &super::util::Client,
    tasks_only: bool,
    active: bool,
    status_only: bool,
    cancel: Option<String>,
    command: &[String],
    output: &str,
    verbose: bool,
) -> Result<(), String> {
    use crate::batch::{BatchResponse, BatchListResponse};

    if let Some(batch_id) = cancel {
        let response = client.post(&format!("/batch/{}/cancel", batch_id))?;
        if output == "json" {
            println!("{}", response);
        } else {
            let batch: BatchResponse =
                serde_json::from_str(&response).map_err(|e| e.to_string())?;
            println!("Cancelled batch {}", batch.id);
        }
        return Ok(());
    }

    if status_only {
        let response = client.get("/batch")?;
        if output == "json" {
            println!("{}", response);
        } else {
            let list: BatchListResponse =
                serde_json::from_str(&response).map_err(|e| e.to_string())?;
            print!("{}", list.render_terminal());
        }
        return Ok(());
    }

    if command.is_empty() {
        return Err("No command specified. Use -- <command...> or --status to list batches.".into());
    }

    // Fetch project list
    let path = if active {
        "/project/list?active=true"
    } else {
        "/project/list"
    };
    let response = client.get(path)?;
    let json: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;

    let projects = json["current"]
        .as_array()
        .ok_or("No projects found")?;

    let runs: Vec<serde_json::Value> = projects
        .iter()
        .filter_map(|p| {
            let key = p["project_key"].as_str()?;
            if tasks_only && !key.contains(':') {
                return None;
            }
            let dir = p["path"].as_str()?;
            Some(serde_json::json!({ "key": key, "dir": dir }))
        })
        .collect();

    if runs.is_empty() {
        return Err("No projects to run command in".into());
    }

    let batch_req = serde_json::json!({
        "command": command,
        "runs": runs,
    });

    let total = runs.len();
    if verbose {
        eprintln!("Starting batch: {} across {} projects", command.join(" "), total);
    }

    let response = client.post_json("/batch", &batch_req)?;
    let mut batch: BatchResponse =
        serde_json::from_str(&response).map_err(|e| e.to_string())?;

    let mut seen_completed = batch.completed;

    loop {
        if batch.completed > seen_completed {
            if verbose {
                eprintln!("[{}/{}]", batch.completed, total);
            }
            seen_completed = batch.completed;
        }

        if batch.done {
            break;
        }

        let poll_path = format!("/batch/{}?completed={}", batch.id, seen_completed);
        let response = client.get_with_wait(&poll_path, 30)?;
        batch = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    }

    if output == "json" {
        println!("{}", serde_json::to_string_pretty(&batch).map_err(|e| e.to_string())?);
    } else {
        print!("{}", batch.render_terminal());
    }

    if batch.runs.iter().any(|r| matches!(r.status, crate::batch::RunStatus::Failed | crate::batch::RunStatus::Cancelled)) {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_project_item_bare_project() {
        // Project without JIRA - just shows project_key
        let item = serde_json::json!({
            "project_key": "wormhole",
            "path": "/Users/dan/src/wormhole"
        });
        let rendered = render_project_item(&item);
        // Should contain the project key (inside a hyperlink)
        assert!(rendered.contains("wormhole"), "Should contain project key");
        // Should not contain emoji (no JIRA)
        assert!(
            !rendered.contains("●"),
            "Should not have indicator without JIRA"
        );
    }

    #[test]
    fn test_render_project_item_task_without_jira() {
        // Task but no JIRA - shows project_key
        let item = serde_json::json!({
            "project_key": "cli:feature-branch",
            "path": "/Users/dan/src/cli/feature-branch"
        });
        let rendered = render_project_item(&item);
        assert!(
            rendered.contains("cli:feature-branch"),
            "Should show project_key"
        );
        assert!(
            !rendered.contains("●"),
            "Should not have indicator without JIRA"
        );
    }

    #[test]
    fn test_render_project_item_task_with_jira() {
        // Task with JIRA info - shows emoji, task, JIRA key (no summary)
        let item = serde_json::json!({
            "project_key": "cli:standalone-activity",
            "path": "/Users/dan/src/cli/standalone-activity",
            "jira": {
                "key": "ACT-107",
                "status": "In Progress",
                "summary": "Standalone activity CLI integration"
            }
        });
        let rendered = render_project_item(&item);
        assert!(rendered.contains("●"), "Should contain status indicator");
        assert!(
            rendered.contains("cli:standalone-activity"),
            "Should contain project_key"
        );
        assert!(rendered.contains("ACT-107"), "Should contain JIRA key");
        assert!(
            !rendered.contains("Standalone activity CLI integration"),
            "Should not contain summary"
        );
    }

    #[test]
    fn test_render_project_item_task_with_pr() {
        // Task with JIRA and PR
        let item = serde_json::json!({
            "project_key": "cli:feature",
            "jira": {
                "key": "ACT-100",
                "status": "In Review",
                "summary": "Feature work"
            },
            "pr": {
                "number": 123,
                "url": "https://github.com/org/cli/pull/123",
                "isDraft": false
            }
        });
        let rendered = render_project_item(&item);
        assert!(rendered.contains("#123"), "Should contain PR number");
    }

    #[test]
    fn test_render_project_item_draft_pr() {
        let item = serde_json::json!({
            "project_key": "cli:feature",
            "jira": {
                "key": "ACT-100",
                "status": "In Review",
                "summary": "Feature work"
            },
            "pr": {
                "number": 456,
                "url": "https://github.com/org/cli/pull/456",
                "isDraft": true
            }
        });
        let rendered = render_project_item(&item);
        assert!(rendered.contains("#456 (draft)"), "Should show draft PR");
    }
}
