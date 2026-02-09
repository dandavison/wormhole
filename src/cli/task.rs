use crate::config;
use crate::jira;
use crate::project::ProjectKey;
use crate::pst::TerminalHyperlink;

use super::util::*;

pub(super) fn task_create_from_sprint(client: &Client) -> Result<(), String> {
    use std::collections::HashMap;

    // Refresh to discover all existing worktrees before checking
    let _ = client.post("/project/refresh-tasks");

    let response = client.get("/project/list")?;
    let parsed: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    let current = parsed
        .get("current")
        .ok_or("Missing 'current' in /project/list response")?
        .as_array()
        .ok_or("'current' is not an array")?;

    // Build maps from the project list
    let mut existing_by_jira: HashMap<String, (String, String)> = HashMap::new();
    let mut has_non_draft_pr: HashMap<String, bool> = HashMap::new();

    for item in current {
        let project_key = item
            .get("project_key")
            .ok_or("Missing 'project_key' in project item")?
            .as_str()
            .ok_or("'project_key' is not a string")?;

        // Track PR status
        let pr_is_non_draft = item
            .get("pr")
            .is_some_and(|pr| !pr.get("isDraft").and_then(|d| d.as_bool()).unwrap_or(false));
        has_non_draft_pr.insert(project_key.to_string(), pr_is_non_draft);

        // Only process items with jira_key
        if let Some(jira_key) = item.get("kv").and_then(|kv| kv.get("jira_key")) {
            let jira_key = jira_key.as_str().ok_or("'jira_key' is not a string")?;
            let (repo, branch) = project_key.split_once(':').ok_or_else(|| {
                format!(
                    "Task with jira_key '{}' has invalid project_key '{}' (expected repo:branch)",
                    jira_key, project_key
                )
            })?;
            existing_by_jira.insert(jira_key.to_string(), (repo.to_string(), branch.to_string()));
        }
    }

    // Map (repo, branch) -> jira_key for reverse lookup
    let existing_by_task: HashMap<(String, String), String> = existing_by_jira
        .iter()
        .map(|(jira, (repo, branch))| ((repo.clone(), branch.clone()), jira.clone()))
        .collect();

    let available_projects = get_available_projects(client)?;
    let issues = jira::get_sprint_issues()?;

    let mut rl = create_project_editor(available_projects)?;
    let mut created_count = 0;
    let mut skipped_count = 0;

    for issue in &issues {
        let existing = existing_by_jira.get(&issue.key);

        // Check if task exists and has non-draft PR
        let has_pr = existing
            .map(|(repo, branch)| {
                let store_key = format!("{}:{}", repo, branch);
                *has_non_draft_pr.get(&store_key).unwrap_or(&false)
            })
            .unwrap_or(false);

        let indicator = jira::status_indicator(&issue.status);

        if let Some(reason) = should_skip_issue(has_pr) {
            println!("{} {} {} [{}]", indicator, issue.key, issue.summary, reason);
            skipped_count += 1;
            continue;
        }

        println!(
            "\n─────────────────────────────────────────────────────────────────────────────────"
        );
        println!(
            "{} {} {} [{}]",
            indicator, issue.key, issue.summary, issue.status
        );

        // If task exists locally, show it and offer to confirm/skip
        if let Some((existing_repo, existing_branch)) = existing {
            let existing_key = ProjectKey::task(existing_repo, existing_branch);
            println!("  Already exists locally: {}", existing_key.hyperlink());
            println!();
            let confirm = match rl.readline("  ▶ Keep existing? [Y/n/q]: ") {
                Ok(line) => line.trim().to_lowercase(),
                Err(rustyline::error::ReadlineError::Interrupted) => "n".to_string(),
                Err(rustyline::error::ReadlineError::Eof) => break,
                Err(e) => return Err(format!("Input error: {}", e)),
            };
            match confirm.as_str() {
                "" | "y" | "yes" => {
                    println!("  Keeping {}", existing_key.hyperlink());
                    skipped_count += 1;
                    continue;
                }
                "q" | "quit" => break,
                _ => {
                    println!("  Will prompt for new location (existing task will remain)");
                }
            }
        }

        // Prompt for home project
        let home = match rl.readline("  home: ") {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    eprintln!("  Skipping (no home)");
                    continue;
                }
                trimmed.to_string()
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("  Skipping");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => return Err(format!("Input error: {}", e)),
        };

        // Get branches from the selected repo for completion
        let branches = config::resolve_project_name(&home)
            .map(|path| crate::git::list_branches(&path))
            .unwrap_or_default();
        let mut branch_rl = create_branch_editor(branches)?;

        // Prompt for branch
        let default_branch = existing
            .map(|(_, branch)| branch.clone())
            .unwrap_or_else(|| to_kebab_case(&issue.summary));

        let branch = match branch_rl.readline_with_initial("  branch: ", (&default_branch, "")) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    default_branch
                } else {
                    trimmed.to_string()
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("  Skipping");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => return Err(format!("Input error: {}", e)),
        };

        // Safety check: warn if this repo:branch already has a different JIRA key
        let task_key_tuple = (home.clone(), branch.clone());
        if let Some(other_jira) = existing_by_task.get(&task_key_tuple) {
            if other_jira != &issue.key {
                let key = ProjectKey::task(&home, &branch);
                eprintln!(
                    "  WARNING: {} is already linked to {}",
                    key.hyperlink(),
                    other_jira
                );
                let confirm = match rl.readline("  Continue anyway? [y/N]: ") {
                    Ok(line) => line.trim().to_lowercase(),
                    Err(_) => "n".to_string(),
                };
                if confirm != "y" && confirm != "yes" {
                    eprintln!("  Skipping to avoid conflict");
                    continue;
                }
            }
        }

        // Safety check: warn if changing repo or branch for existing JIRA task
        if let Some((existing_repo, existing_branch)) = existing {
            if &home != existing_repo || &branch != existing_branch {
                let existing_key = ProjectKey::task(existing_repo, existing_branch);
                let new_key = ProjectKey::task(&home, &branch);
                eprintln!(
                    "  WARNING: {} already exists as {}",
                    issue.key,
                    existing_key.hyperlink()
                );
                eprintln!(
                    "  Creating {} will result in duplicate tasks for same JIRA",
                    new_key.hyperlink()
                );
                let confirm = match rl.readline("  Continue anyway? [y/N]: ") {
                    Ok(line) => line.trim().to_lowercase(),
                    Err(_) => "n".to_string(),
                };
                if confirm != "y" && confirm != "yes" {
                    eprintln!("  Skipping to avoid duplicate");
                    continue;
                }
            }
        }

        // Final confirmation before creating
        let task_key = ProjectKey::task(&home, &branch);
        println!("  Creating {} for {}", task_key.hyperlink(), issue.key);
        upsert_task(client, &home, &branch, Some(&issue.key))?;
        println!("  Created {}", task_key.hyperlink());
        created_count += 1;
    }

    println!(
        "\nDone: {} created, {} skipped",
        created_count, skipped_count
    );

    if created_count > 0 {
        let _ = client.post("/project/refresh");
    }

    Ok(())
}

/// Represents a parsed task target for the upsert command
enum UpsertTarget {
    /// A project key like "repo:branch"
    ProjectKey { repo: String, branch: String },
    /// A JIRA key (bare like "ACT-123" or extracted from URL)
    JiraKey(String),
}

/// Find an existing task by JIRA key from the project list
fn find_task_by_jira_key(
    client: &Client,
    jira_key: &str,
) -> Result<Option<(String, String)>, String> {
    let response = client.get("/project/list")?;
    let parsed: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;

    if let Some(current) = parsed.get("current").and_then(|v| v.as_array()) {
        for item in current {
            if let Some(kv_jira) = item
                .get("kv")
                .and_then(|kv| kv.get("jira_key"))
                .and_then(|k| k.as_str())
            {
                if kv_jira == jira_key {
                    if let Some(project_key) = item.get("project_key").and_then(|k| k.as_str()) {
                        if let Some((repo, branch)) = project_key.split_once(':') {
                            return Ok(Some((repo.to_string(), branch.to_string())));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

/// Get existing task info by project key
fn get_task_info(client: &Client, repo: &str, branch: &str) -> Result<Option<String>, String> {
    let store_key = format!("{}:{}", repo, branch);
    let kv_url = format!("/kv/{}/jira_key", store_key);
    match client.get(&kv_url) {
        Ok(jira_key) => Ok(Some(jira_key)),
        Err(_) => Ok(None),
    }
}

pub(super) fn task_upsert(
    client: &Client,
    target: &str,
    home_project: Option<String>,
) -> Result<(), String> {
    // Refresh to get latest task list
    let _ = client.post("/project/refresh-tasks");

    // Parse target to determine what we're working with
    let (upsert_target, existing_task) = parse_upsert_target(client, target)?;

    // Get JIRA info if we have a JIRA key
    let (jira_key, jira_issue) = match &upsert_target {
        UpsertTarget::JiraKey(key) => {
            let issue = jira::get_issue(key)?;
            (Some(key.clone()), issue)
        }
        UpsertTarget::ProjectKey { repo, branch } => {
            // Check if existing task has a JIRA key
            match get_task_info(client, repo, branch)? {
                Some(key) => {
                    let issue = jira::get_issue(&key)?;
                    (Some(key), issue)
                }
                None => (None, None),
            }
        }
    };

    // Print header with JIRA info if available
    if let Some(ref key) = jira_key {
        if let Some(ref issue) = jira_issue {
            println!("{} {}", key, issue.summary);
        } else {
            println!("{}", key);
        }
    }

    // Determine defaults
    let (default_home, default_branch) = match (&existing_task, &upsert_target) {
        (Some((repo, branch)), _) => (repo.clone(), branch.clone()),
        (None, UpsertTarget::ProjectKey { repo, branch }) => (repo.clone(), branch.clone()),
        (None, UpsertTarget::JiraKey(_)) => {
            let home = home_project
                .clone()
                .or_else(|| std::env::var("WORMHOLE_DEFAULT_HOME_PROJECT").ok())
                .unwrap_or_default();
            let branch = jira_issue
                .as_ref()
                .map(|i| to_kebab_case(&i.summary))
                .unwrap_or_default();
            (home, branch)
        }
    };

    let available_projects = get_available_projects(client)?;
    let mut rl = create_project_editor(available_projects)?;

    // Prompt for home project
    let home = if let Some(h) = home_project {
        h
    } else {
        match rl.readline_with_initial("home: ", (&default_home, "")) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    if default_home.is_empty() {
                        return Err("Home project is required".to_string());
                    }
                    default_home.clone()
                } else {
                    trimmed.to_string()
                }
            }
            Err(_) => return Err("Aborted".to_string()),
        }
    };

    // Get branches from the selected repo for completion
    let branches = config::resolve_project_name(&home)
        .map(|path| crate::git::list_branches(&path))
        .unwrap_or_default();
    let mut branch_rl = create_branch_editor(branches)?;

    // Prompt for branch
    let branch = match branch_rl.readline_with_initial("branch: ", (&default_branch, "")) {
        Ok(line) => {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if default_branch.is_empty() {
                    return Err("Branch is required".to_string());
                }
                default_branch.clone()
            } else {
                trimmed.to_string()
            }
        }
        Err(_) => return Err("Aborted".to_string()),
    };

    let task_key = ProjectKey::task(&home, &branch);
    let same_location = existing_task
        .as_ref()
        .is_some_and(|(r, b)| r == &home && b == &branch);
    let is_move = existing_task.is_some() && !same_location;

    if same_location {
        println!("Updating {}", task_key.hyperlink());
    } else if is_move {
        let (old_repo, old_branch) = existing_task.as_ref().unwrap();
        let old_key = ProjectKey::task(old_repo, old_branch);
        println!("Moving {} → {}", old_key.hyperlink(), task_key.hyperlink());
    } else {
        println!("Creating {}", task_key.hyperlink());
    }

    // Create/ensure new task exists
    upsert_task(client, &home, &branch, jira_key.as_deref())?;

    // Delete old worktree if moving to a new location
    if is_move {
        let (old_repo, old_branch) = existing_task.as_ref().unwrap();
        let old_key = format!("{}:{}", old_repo, old_branch);
        if let Err(e) = client.post(&format!("/project/remove/{}", old_key)) {
            eprintln!("Warning: failed to remove old worktree: {}", e);
        }
    }

    // Refresh cache
    let _ = client.post("/project/refresh");

    if same_location {
        println!("Updated {}", task_key.hyperlink());
    } else if is_move {
        println!("Moved to {}", task_key.hyperlink());
    } else if let Some(ref key) = jira_key {
        println!("Created task {} for {}", task_key.hyperlink(), key);
    } else {
        println!("Created task {}", task_key.hyperlink());
    }

    Ok(())
}

fn parse_upsert_target(
    client: &Client,
    target: &str,
) -> Result<(UpsertTarget, Option<(String, String)>), String> {
    // First, check if it's a JIRA URL or key
    if let Some(jira_key) = crate::handlers::describe::parse_jira_key_or_url(target) {
        let existing = find_task_by_jira_key(client, &jira_key)?;
        return Ok((UpsertTarget::JiraKey(jira_key), existing));
    }

    // Check if it's a project key (repo:branch)
    if let Some((repo, branch)) = target.split_once(':') {
        // Verify the task exists (or at least the repo exists)
        let existing = if client
            .get(&format!("/kv/{}:{}/jira_key", repo, branch))
            .is_ok()
        {
            Some((repo.to_string(), branch.to_string()))
        } else {
            None
        };
        return Ok((
            UpsertTarget::ProjectKey {
                repo: repo.to_string(),
                branch: branch.to_string(),
            },
            existing,
        ));
    }

    Err(format!(
        "Could not parse target '{}'. Expected: project key (repo:branch), JIRA URL, or JIRA key (ACT-123)",
        target
    ))
}

/// Create or update a task with optional JIRA key
fn upsert_task(
    client: &Client,
    home: &str,
    branch: &str,
    jira_key: Option<&str>,
) -> Result<(), String> {
    // Create the worktree/task
    let url = format!("/project/create/{}?home-project={}", branch, home);
    client.get(&url)?;

    // Store JIRA key if provided
    if let Some(key) = jira_key {
        let store_key = format!("{}:{}", home, branch);
        let kv_url = format!("/kv/{}/jira_key", store_key);
        let _ = client.put(&kv_url, key);
    }

    Ok(())
}
