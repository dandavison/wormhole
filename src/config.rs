use crate::editor::Editor;
use crate::terminal::Terminal;
use glob::Pattern;
use serde::Deserialize;
use std::sync::OnceLock;

pub const TERMINAL: Terminal = Terminal::Alacritty { tmux: true };

static EDITOR: OnceLock<Editor> = OnceLock::new();

pub fn editor() -> &'static Editor {
    EDITOR.get_or_init(|| match std::env::var("WORMHOLE_EDITOR").ok().as_deref() {
        Some("none") => Editor::None,
        Some("cursor") | None => Editor::Cursor,
        Some("code") => Editor::VSCode,
        Some("code-insiders") => Editor::VSCodeInsiders,
        Some("emacs") => Editor::Emacs,
        Some("idea") => Editor::IntelliJ,
        Some("pycharm") => Editor::PyCharm,
        _ => Editor::Cursor,
    })
}

// This port number is currently hardcoded in http clients such as the MacOS GUI
// app and the CLI utilities under cli/.
// Can be overridden with WORMHOLE_PORT environment variable for testing
static PORT: OnceLock<u16> = OnceLock::new();

pub fn wormhole_port() -> u16 {
    *PORT.get_or_init(|| {
        std::env::var("WORMHOLE_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(7117)
    })
}

/// Returns directories to search for projects, from WORMHOLE_PATH env var.
/// Format is colon-separated like PATH.
pub fn search_paths() -> Vec<std::path::PathBuf> {
    std::env::var("WORMHOLE_PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .collect()
}

/// Config from .wormhole.toml file
#[derive(Debug, Deserialize, Default)]
pub struct WormholeConfig {
    #[serde(default)]
    pub available: AvailableConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct AvailableConfig {
    /// Glob patterns to exclude from available projects
    #[serde(default)]
    pub exclude: Vec<String>,
}

static CONFIG: OnceLock<WormholeConfig> = OnceLock::new();

pub fn config() -> &'static WormholeConfig {
    CONFIG.get_or_init(|| {
        let config_path = std::env::current_dir()
            .ok()
            .map(|p| p.join(".wormhole.toml"));

        if let Some(path) = config_path {
            if path.exists() {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(config) = toml::from_str(&contents) {
                        return config;
                    }
                }
            }
        }
        WormholeConfig::default()
    })
}

/// Check if a project name should be excluded based on config
pub fn is_excluded(name: &str) -> bool {
    config().available.exclude.iter().any(|pattern| {
        Pattern::new(pattern)
            .map(|p| p.matches(name))
            .unwrap_or(false)
    })
}

/// Returns available project names mapped to their paths.
/// When the same directory name appears in multiple search paths,
/// the first occurrence gets the simple name, and later occurrences
/// get `<parent-dir>-<name>` format.
pub fn available_projects() -> std::collections::BTreeMap<String, std::path::PathBuf> {
    available_projects_from_paths(&search_paths())
}

fn available_projects_from_paths(
    search_paths: &[std::path::PathBuf],
) -> std::collections::BTreeMap<String, std::path::PathBuf> {
    use std::collections::{BTreeMap, HashSet};

    let mut result: BTreeMap<String, std::path::PathBuf> = BTreeMap::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for search_dir in search_paths {
        let parent_name = search_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if let Ok(entries) = std::fs::read_dir(search_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(dir_name) = entry.file_name().to_str().map(String::from) else {
                    continue;
                };
                if dir_name.starts_with('.') || is_excluded(&dir_name) {
                    continue;
                }
                // Skip git worktrees - they're tasks, not projects
                if crate::git::is_worktree(&path) {
                    continue;
                }

                if seen_names.contains(&dir_name) {
                    let prefixed = format!("{}-{}", parent_name, dir_name);
                    result.entry(prefixed).or_insert(path);
                } else {
                    seen_names.insert(dir_name.clone());
                    result.insert(dir_name, path);
                }
            }
        }
    }
    result
}

/// Resolve a project name to its path, handling both simple names
/// and prefixed names like `devenv-temporal`.
pub fn resolve_project_name(name: &str) -> Option<std::path::PathBuf> {
    available_projects().get(name).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_first_occurrence_gets_simple_name() {
        let temp = TempDir::new().unwrap();
        let dir1 = temp.path().join("repos");
        let dir2 = temp.path().join("devenv");
        std::fs::create_dir_all(dir1.join("temporal")).unwrap();
        std::fs::create_dir_all(dir2.join("temporal")).unwrap();

        let paths = vec![dir1.clone(), dir2.clone()];
        let projects = available_projects_from_paths(&paths);

        assert_eq!(
            projects.get("temporal"),
            Some(&dir1.join("temporal")),
            "First occurrence should get simple name"
        );
        assert_eq!(
            projects.get("devenv-temporal"),
            Some(&dir2.join("temporal")),
            "Later occurrence should get prefixed name"
        );
    }

    #[test]
    fn test_no_clash_keeps_simple_names() {
        let temp = TempDir::new().unwrap();
        let dir1 = temp.path().join("repos");
        let dir2 = temp.path().join("devenv");
        std::fs::create_dir_all(dir1.join("project-a")).unwrap();
        std::fs::create_dir_all(dir2.join("project-b")).unwrap();

        let paths = vec![dir1.clone(), dir2.clone()];
        let projects = available_projects_from_paths(&paths);

        assert_eq!(projects.get("project-a"), Some(&dir1.join("project-a")));
        assert_eq!(projects.get("project-b"), Some(&dir2.join("project-b")));
        assert!(projects.get("repos-project-a").is_none());
        assert!(projects.get("devenv-project-b").is_none());
    }

    #[test]
    fn test_hidden_dirs_excluded() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("repos");
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        std::fs::create_dir_all(dir.join("visible")).unwrap();

        let paths = vec![dir.clone()];
        let projects = available_projects_from_paths(&paths);

        assert!(projects.get(".hidden").is_none());
        assert_eq!(projects.get("visible"), Some(&dir.join("visible")));
    }

    #[test]
    fn test_three_way_clash() {
        let temp = TempDir::new().unwrap();
        let dir1 = temp.path().join("repos");
        let dir2 = temp.path().join("devenv");
        let dir3 = temp.path().join("work");
        std::fs::create_dir_all(dir1.join("temporal")).unwrap();
        std::fs::create_dir_all(dir2.join("temporal")).unwrap();
        std::fs::create_dir_all(dir3.join("temporal")).unwrap();

        let paths = vec![dir1.clone(), dir2.clone(), dir3.clone()];
        let projects = available_projects_from_paths(&paths);

        assert_eq!(projects.get("temporal"), Some(&dir1.join("temporal")));
        assert_eq!(
            projects.get("devenv-temporal"),
            Some(&dir2.join("temporal"))
        );
        assert_eq!(projects.get("work-temporal"), Some(&dir3.join("temporal")));
    }
}
