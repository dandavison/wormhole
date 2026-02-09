use serde::Serialize;

use crate::config;
use crate::project::ProjectKey;
use crate::pst::TerminalHyperlink;

#[derive(Serialize)]
struct PersistedDataReport {
    projects: Vec<ProjectPersistedData>,
}

#[derive(Serialize)]
struct ProjectPersistedData {
    name: String,
    path: String,
    worktrees: Vec<WorktreeInfo>,
    kv: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

#[derive(Serialize)]
struct WorktreeInfo {
    dir: String,
    branch: Option<String>,
}

impl PersistedDataReport {
    fn render_terminal(&self) -> String {
        if self.projects.is_empty() {
            return "No persisted wormhole data found.".to_string();
        }

        let mut lines = Vec::new();
        for project in &self.projects {
            let name_linked = ProjectKey::parse(&project.name).hyperlink();
            lines.push(format!("{}:", name_linked));
            lines.push(format!("  path: {}", project.path));

            if !project.worktrees.is_empty() {
                lines.push("  worktrees:".to_string());
                for wt in &project.worktrees {
                    let branch = wt.branch.as_deref().unwrap_or("(detached)");
                    lines.push(format!("    {} -> {}", wt.dir, branch));
                }
            }

            if !project.kv.is_empty() {
                lines.push("  kv:".to_string());
                for (file, pairs) in &project.kv {
                    lines.push(format!("    {}:", file));
                    for (k, v) in pairs {
                        lines.push(format!("      {}: {}", k, v));
                    }
                }
            }
            lines.push(String::new());
        }
        lines.join("\n")
    }
}

pub(super) fn doctor_persisted_data(output: &str) -> Result<(), String> {
    use rayon::prelude::*;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    // Discover all available projects
    let available = config::available_projects();
    let repo_paths: Vec<(String, PathBuf)> = available.into_iter().collect();

    // Query each repo in parallel - only include repos with wormhole data
    let projects: Vec<ProjectPersistedData> = repo_paths
        .par_iter()
        .filter_map(|(name, path)| {
            if !crate::git::is_git_repo(path) {
                return None;
            }

            // Get worktrees
            let worktrees = crate::git::list_worktrees(path);
            let worktree_base = crate::git::worktree_base_path(path);
            let wormhole_worktrees: Vec<WorktreeInfo> = worktrees
                .into_iter()
                .filter(|wt| wt.path.starts_with(&worktree_base))
                .map(|wt| WorktreeInfo {
                    dir: wt
                        .path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?")
                        .to_string(),
                    branch: wt.branch,
                })
                .collect();

            // Read KV files
            let kv_dir = crate::git::git_common_dir(path).join("wormhole/kv");
            let mut all_kv: HashMap<String, HashMap<String, String>> = HashMap::new();
            if let Ok(entries) = fs::read_dir(&kv_dir) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if file_path.extension().map(|e| e == "json").unwrap_or(false) {
                        if let Ok(contents) = fs::read_to_string(&file_path) {
                            if let Ok(kv) =
                                serde_json::from_str::<HashMap<String, String>>(&contents)
                            {
                                let stem = file_path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                all_kv.insert(stem, kv);
                            }
                        }
                    }
                }
            }

            // Only include if there's wormhole data
            if wormhole_worktrees.is_empty() && all_kv.is_empty() {
                return None;
            }

            Some(ProjectPersistedData {
                name: name.clone(),
                path: path.display().to_string(),
                worktrees: wormhole_worktrees,
                kv: all_kv,
            })
        })
        .collect();

    let report = PersistedDataReport { projects };

    if output == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
    } else {
        println!("{}", report.render_terminal());
    }

    Ok(())
}

pub(super) fn doctor_migrate_worktrees() -> Result<(), String> {
    let available = config::available_projects();
    let mut total = 0;
    for (name, path) in &available {
        if !crate::git::is_git_repo(path) {
            continue;
        }
        match crate::git::migrate_worktrees(name, path) {
            Ok(0) => {}
            Ok(n) => {
                println!("{}: migrated {} worktree(s)", name, n);
                total += n;
            }
            Err(e) => eprintln!("{}: error: {}", name, e),
        }
    }
    if total == 0 {
        println!("No worktrees needed migration.");
    } else {
        println!("\nMigrated {} worktree(s) total.", total);
    }
    Ok(())
}
