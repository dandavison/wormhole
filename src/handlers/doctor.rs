use std::collections::HashMap;
use std::fs;

use hyper::{Body, Response, StatusCode};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::tty::TerminalHyperlink;
use crate::{config, git, task};

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

    let worktree_dir = config::worktree_dir();
    let results: Vec<ConformTaskResult> = repo_paths
        .par_iter()
        .filter(|(_, path)| git::is_git_repo(path))
        .flat_map(|(name, path)| {
            let worktree_base = worktree_dir.join(name);
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
        .flat_map(|(name, path)| {
            git::find_orphan_worktree_dirs(path, &worktree_dir.join(name))
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

pub fn persisted_data() -> Response<Body> {
    let available = config::available_projects();
    let repo_paths: Vec<_> = available.into_iter().collect();
    let worktree_dir = config::worktree_dir();

    let projects: Vec<ProjectPersistedData> = repo_paths
        .par_iter()
        .filter_map(|(name, path)| {
            if !git::is_git_repo(path) {
                return None;
            }

            let worktrees = git::list_worktrees(path);
            let worktree_base = worktree_dir.join(name);
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

// --- list-editor-windows ---

#[derive(Serialize, Deserialize)]
pub struct EditorWindowsReport {
    pub editor: String,
    pub windows: Vec<EditorWindow>,
}

#[derive(Serialize, Deserialize)]
pub struct EditorWindow {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub visible: bool,
    pub screen: String,
}

impl EditorWindowsReport {
    pub fn render_terminal(&self) -> String {
        use crate::project::ProjectKey;
        self.windows
            .iter()
            .map(|w| match &w.project {
                Some(key) => ProjectKey::parse(key).hyperlink(),
                None => w.title.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn list_editor_windows() -> Response<Body> {
    let editor = config::editor();
    let app_name = editor.application_name();
    if app_name.is_empty() {
        return json_response(&EditorWindowsReport {
            editor: "none".to_string(),
            windows: vec![],
        });
    }

    // Build lookup from encoded workspace name → project key.
    // Workspace files encode `/` as `--` in the filename, which becomes
    // the window title. This isn't reversible (branch names may contain
    // `--`), so we match against known project keys.
    let encoded_to_key: HashMap<String, String> = {
        let projects = crate::projects::lock();
        projects
            .keys()
            .into_iter()
            .map(|k| {
                let key_str = k.to_string();
                let encoded = key_str.replace('/', "--");
                (encoded, key_str)
            })
            .collect()
    };

    let lua = format!(
        r#"
        local app = hs.application.find("{app_name}")
        if app then
            for _, win in ipairs(app:allWindows()) do
                local title = win:title()
                if title and title ~= "" then
                    local visible = win:isVisible() and "1" or "0"
                    local screen = win:screen():name() or ""
                    print(title .. "\t" .. visible .. "\t" .. screen)
                end
            end
        end
    "#
    );
    let output = crate::hammerspoon::execute(&lua);
    let windows: Vec<EditorWindow> = String::from_utf8_lossy(&output)
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let title = parts.next()?.to_string();
            let visible = parts.next().map(|v| v == "1").unwrap_or(true);
            let screen = parts.next().unwrap_or("").to_string();
            let project = parse_workspace_name(&title)
                .and_then(|ws| encoded_to_key.get(ws))
                .cloned();
            Some(EditorWindow {
                title,
                project,
                visible,
                screen,
            })
        })
        .collect();
    json_response(&EditorWindowsReport {
        editor: app_name.to_string(),
        windows,
    })
}

/// Extract workspace name from a VSCode/Cursor window title.
///
/// Titles follow the pattern: `[● ][<tab-title> — ]<workspace> (Workspace)`
/// Returns the workspace name, or None for non-workspace windows.
fn parse_workspace_name(title: &str) -> Option<&str> {
    let rest = title.strip_suffix(" (Workspace)")?;
    // The workspace name is after the last ` — ` (em dash), or the
    // whole string if there's no em dash (no active tab title).
    let ws = rest.rsplit_once(" \u{2014} ").map_or(rest, |(_, ws)| ws);
    Some(ws)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_workspace_name() {
        // filename — workspace
        assert_eq!(
            parse_workspace_name("init.zsh — shell-config (Workspace)"),
            Some("shell-config")
        );
        // workspace only (no active tab)
        assert_eq!(
            parse_workspace_name("sdk-typescript (Workspace)"),
            Some("sdk-typescript")
        );
        // unsaved file — workspace
        assert_eq!(
            parse_workspace_name("● client.go — cli (Workspace)"),
            Some("cli")
        );
        // task with encoded branch
        assert_eq!(
            parse_workspace_name("cli:release--v1.6.x-standalone-activity (Workspace)"),
            Some("cli:release--v1.6.x-standalone-activity")
        );
        // truncated text — workspace
        assert_eq!(
            parse_workspace_name("Good morning. This is th… — bat (Workspace)"),
            Some("bat")
        );
        // non-workspace window
        assert_eq!(
            parse_workspace_name("● mcp-resource-1771262755954.txt"),
            None
        );
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
