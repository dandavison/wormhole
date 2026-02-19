use std::collections::HashMap;
use std::fs;

use hyper::{Body, Response, StatusCode};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::pst::TerminalHyperlink;
use crate::{config, editor, git, task};

// --- conform ---

#[derive(Serialize, Deserialize)]
pub struct ConformResult {
    pub dry_run: bool,
    pub results: Vec<ConformTaskResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub orphans_removed: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ConformTaskResult {
    pub task: String,
    pub actions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ConformResult {
    pub fn render_terminal(&self) -> String {
        let mut lines = Vec::new();
        let mut ok = 0;
        let mut errs = 0;
        for r in &self.results {
            if let Some(ref e) = r.error {
                lines.push(format!("  {} error: {}", r.task, e));
                errs += 1;
            } else {
                let key = crate::project::ProjectKey::parse(&r.task);
                lines.push(format!("  {}", key.hyperlink()));
                for action in &r.actions {
                    lines.push(format!("    {}", action));
                }
                ok += 1;
            }
        }
        for orphan in &self.orphans_removed {
            let verb = if self.dry_run {
                "would remove"
            } else {
                "removed"
            };
            lines.push(format!("  {} orphan: {}", verb, orphan));
        }
        let verb = if self.dry_run {
            "Would conform"
        } else {
            "Conformed"
        };
        let mut summary = format!("{} {} task(s), {} error(s)", verb, ok, errs);
        if !self.orphans_removed.is_empty() {
            summary.push_str(&format!(", {} orphan(s)", self.orphans_removed.len()));
        }
        summary.push('.');
        lines.push(summary);
        lines.join("\n")
    }
}

pub fn conform(dry_run: bool) -> Response<Body> {
    let available = config::available_projects();
    let repo_paths: Vec<_> = available.into_iter().collect();

    let results: Vec<ConformTaskResult> = repo_paths
        .par_iter()
        .filter(|(_, path)| git::is_git_repo(path))
        .flat_map(|(name, path)| {
            let worktree_base = git::worktree_base_path(path);
            git::list_worktrees(path)
                .into_iter()
                .filter(|wt| wt.path.starts_with(&worktree_base))
                .filter_map(|wt| {
                    let branch = wt.branch.as_deref()?;
                    let task_key = format!("{}:{}", name, branch);
                    match task::conform_task_worktree(&wt.path, name, branch, dry_run) {
                        Ok(actions) => Some(ConformTaskResult {
                            task: task_key,
                            actions,
                            error: None,
                        }),
                        Err(e) => Some(ConformTaskResult {
                            task: task_key,
                            actions: vec![],
                            error: Some(e),
                        }),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let orphans_removed: Vec<String> = repo_paths
        .par_iter()
        .filter(|(_, path)| git::is_git_repo(path))
        .flat_map(|(_, path)| {
            git::find_orphan_worktree_dirs(path)
                .into_iter()
                .filter_map(|orphan| {
                    let display = orphan.display().to_string();
                    if !dry_run {
                        fs::remove_dir_all(&orphan).ok()?;
                        // Remove empty branch parent directory
                        if let Some(parent) = orphan.parent() {
                            let _ = fs::remove_dir(parent);
                        }
                    }
                    Some(display)
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let result = ConformResult {
        dry_run,
        results,
        orphans_removed,
    };
    json_response(&result)
}

// --- persisted-data ---

#[derive(Serialize, Deserialize)]
pub struct PersistedDataReport {
    pub projects: Vec<ProjectPersistedData>,
}

#[derive(Serialize, Deserialize)]
pub struct ProjectPersistedData {
    pub name: String,
    pub path: String,
    pub worktrees: Vec<WorktreeInfo>,
    pub kv: HashMap<String, HashMap<String, String>>,
}

#[derive(Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub dir: String,
    pub branch: Option<String>,
}

impl PersistedDataReport {
    pub fn render_terminal(&self) -> String {
        if self.projects.is_empty() {
            return "No persisted wormhole data found.".to_string();
        }

        let mut lines = Vec::new();
        for project in &self.projects {
            let name_linked = crate::project::ProjectKey::parse(&project.name).hyperlink();
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

fn collect_json_files_recursive(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut result = vec![];
    collect_json_files_inner(dir, &mut result);
    result
}

fn collect_json_files_inner(dir: &std::path::Path, result: &mut Vec<std::path::PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(d) => d,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json_files_inner(&path, result);
        } else if path.extension().map_or(false, |e| e == "json") {
            result.push(path);
        }
    }
}

pub fn persisted_data() -> Response<Body> {
    let available = config::available_projects();
    let repo_paths: Vec<_> = available.into_iter().collect();

    let projects: Vec<ProjectPersistedData> = repo_paths
        .par_iter()
        .filter_map(|(name, path)| {
            if !git::is_git_repo(path) {
                return None;
            }

            let worktrees = git::list_worktrees(path);
            let worktree_base = git::worktree_base_path(path);
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

            let kv_dir = git::git_common_dir(path).join("wormhole/kv");
            let mut all_kv: HashMap<String, HashMap<String, String>> = HashMap::new();
            for file_path in collect_json_files_recursive(&kv_dir) {
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
    json_response(&report)
}

// --- migrate-worktrees ---

#[derive(Serialize, Deserialize)]
pub struct MigrateResult {
    pub projects: Vec<MigrateProjectResult>,
    pub worktrees_total: usize,
    pub workspaces_total: usize,
}

#[derive(Serialize, Deserialize)]
pub struct MigrateProjectResult {
    pub name: String,
    pub worktrees_migrated: usize,
    pub workspaces_migrated: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_error: Option<String>,
}

impl MigrateResult {
    pub fn render_terminal(&self) -> String {
        let mut lines = Vec::new();
        for p in &self.projects {
            if p.worktrees_migrated > 0 {
                lines.push(format!(
                    "{}: migrated {} worktree(s)",
                    p.name, p.worktrees_migrated
                ));
            }
            if p.workspaces_migrated > 0 {
                lines.push(format!(
                    "{}: migrated {} workspace file(s)",
                    p.name, p.workspaces_migrated
                ));
            }
            if let Some(ref e) = p.worktree_error {
                lines.push(format!("{}: worktree error: {}", p.name, e));
            }
            if let Some(ref e) = p.workspace_error {
                lines.push(format!("{}: workspace error: {}", p.name, e));
            }
        }
        if self.worktrees_total == 0 && self.workspaces_total == 0 {
            lines.push("No worktrees or workspace files needed migration.".to_string());
        } else {
            if self.worktrees_total > 0 {
                lines.push(format!(
                    "\nMigrated {} worktree(s) total.",
                    self.worktrees_total
                ));
            }
            if self.workspaces_total > 0 {
                lines.push(format!(
                    "Migrated {} workspace file(s) total.",
                    self.workspaces_total
                ));
            }
        }
        lines.join("\n")
    }
}

pub fn migrate_worktrees() -> Response<Body> {
    let available = config::available_projects();
    let mut projects = Vec::new();
    let mut worktrees_total = 0;
    let mut workspaces_total = 0;

    for (name, path) in &available {
        if !git::is_git_repo(path) {
            continue;
        }
        let mut result = MigrateProjectResult {
            name: name.clone(),
            worktrees_migrated: 0,
            workspaces_migrated: 0,
            worktree_error: None,
            workspace_error: None,
        };
        match git::migrate_worktrees(name, path) {
            Ok(n) => {
                result.worktrees_migrated = n;
                worktrees_total += n;
            }
            Err(e) => result.worktree_error = Some(e),
        }
        match editor::migrate_workspace_files(path) {
            Ok(n) => {
                result.workspaces_migrated += n;
                workspaces_total += n;
            }
            Err(e) => result.workspace_error = Some(e),
        }
        let common = git::git_common_dir(path);
        let ws_dir = common.join("wormhole/workspaces");
        let kv_dir = common.join("wormhole/kv");
        for dir in [&ws_dir, &kv_dir] {
            if let Ok(n) = git::migrate_percent_encoded_files(dir) {
                result.workspaces_migrated += n;
                workspaces_total += n;
            }
        }
        if result.worktrees_migrated > 0
            || result.workspaces_migrated > 0
            || result.worktree_error.is_some()
            || result.workspace_error.is_some()
        {
            projects.push(result);
        }
    }

    let result = MigrateResult {
        projects,
        worktrees_total,
        workspaces_total,
    };
    json_response(&result)
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
