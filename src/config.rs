use crate::editor::Editor;
use crate::terminal::Terminal;
use glob::Pattern;
use serde::Deserialize;
use std::path::{Path, PathBuf};
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

static PORT: OnceLock<u16> = OnceLock::new();

pub fn wormhole_port() -> u16 {
    *PORT.get_or_init(|| {
        std::env::var("WORMHOLE_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(7117)
    })
}

// --- Global config: ~/.wormhole/wormhole.toml ---

#[derive(Debug, Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    search_paths: Vec<SearchPathEntry>,
    worktree_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SearchPathEntry {
    Simple(String),
    WithOptions {
        path: String,
        #[serde(default)]
        exclude: Vec<String>,
    },
}

impl SearchPathEntry {
    fn path(&self) -> &str {
        match self {
            SearchPathEntry::Simple(p) => p,
            SearchPathEntry::WithOptions { path, .. } => path,
        }
    }

    fn exclude(&self) -> &[String] {
        match self {
            SearchPathEntry::Simple(_) => &[],
            SearchPathEntry::WithOptions { exclude, .. } => exclude,
        }
    }
}

struct ResolvedConfig {
    search_paths: Vec<ResolvedSearchPath>,
    worktree_dir: PathBuf,
}

pub struct ResolvedSearchPath {
    pub path: PathBuf,
    pub exclude: Vec<String>,
}

static CONFIG: OnceLock<ResolvedConfig> = OnceLock::new();

fn config() -> &'static ResolvedConfig {
    CONFIG.get_or_init(load_config)
}

fn load_config() -> ResolvedConfig {
    let file = load_config_file();

    let search_paths = if let Ok(env_val) = std::env::var("WORMHOLE_SEARCH_PATHS") {
        env_val
            .split(':')
            .filter(|s| !s.is_empty())
            .map(|s| ResolvedSearchPath {
                path: expand_tilde(s),
                exclude: vec![],
            })
            .collect()
    } else {
        file.search_paths
            .iter()
            .map(|entry| ResolvedSearchPath {
                path: expand_tilde(entry.path()),
                exclude: entry.exclude().to_vec(),
            })
            .collect()
    };

    let worktree_dir = std::env::var("WORMHOLE_WORKTREE_DIR")
        .ok()
        .map(|s| expand_tilde(&s))
        .or_else(|| file.worktree_dir.as_deref().map(expand_tilde))
        .unwrap_or_else(default_worktree_dir);

    ResolvedConfig {
        search_paths,
        worktree_dir,
    }
}

fn load_config_file() -> ConfigFile {
    let Some(home) = dirs::home_dir() else {
        return ConfigFile::default();
    };
    let path = home.join(".wormhole/wormhole.toml");
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return ConfigFile::default();
    };
    toml::from_str(&contents).unwrap_or_default()
}

fn default_worktree_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("worktrees")
}

fn expand_tilde(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(s)
}

pub fn search_paths() -> Vec<&'static ResolvedSearchPath> {
    config().search_paths.iter().collect()
}

pub fn worktree_dir() -> &'static Path {
    &config().worktree_dir
}

pub fn is_excluded(name: &str, search_dir: &Path) -> bool {
    for sp in &config().search_paths {
        if sp.path == search_dir {
            return sp.exclude.iter().any(|pattern| {
                Pattern::new(pattern)
                    .map(|p| p.matches(name))
                    .unwrap_or(false)
            });
        }
    }
    false
}

pub fn available_projects() -> std::collections::BTreeMap<String, PathBuf> {
    let paths: Vec<PathBuf> = search_paths().iter().map(|sp| sp.path.clone()).collect();
    available_projects_from_paths(&paths)
}

fn available_projects_from_paths(
    search_paths: &[PathBuf],
) -> std::collections::BTreeMap<String, PathBuf> {
    use std::collections::{BTreeMap, HashSet};

    let mut result: BTreeMap<String, PathBuf> = BTreeMap::new();
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
                if dir_name.starts_with('.') || is_excluded(&dir_name, search_dir) {
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

pub fn resolve_project_name(name: &str) -> Option<PathBuf> {
    available_projects().get(name).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn git_init(path: &Path) {
        Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(path)
            .output()
            .unwrap();
    }

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

    #[test]
    fn test_worktrees_excluded() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("src");
        std::fs::create_dir_all(&dir).unwrap();

        let repo = dir.join("my-repo");
        std::fs::create_dir_all(&repo).unwrap();
        git_init(&repo);

        let worktree = dir.join("my-branch");
        Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                "my-branch",
                worktree.to_str().unwrap(),
            ])
            .current_dir(&repo)
            .output()
            .unwrap();

        let paths = vec![dir.clone()];
        let projects = available_projects_from_paths(&paths);

        assert_eq!(projects.get("my-repo"), Some(&repo));
        assert!(
            projects.get("my-branch").is_none(),
            "Worktrees should not appear as projects"
        );
    }

    #[test]
    fn test_submodules_included() {
        let temp = TempDir::new().unwrap();

        let parent = temp.path().join("parent");
        std::fs::create_dir_all(&parent).unwrap();
        git_init(&parent);

        let child_src = temp.path().join("child-src");
        std::fs::create_dir_all(&child_src).unwrap();
        git_init(&child_src);

        let repos_dir = parent.join("repos");
        std::fs::create_dir_all(&repos_dir).unwrap();
        Command::new("git")
            .args([
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_src.to_str().unwrap(),
                "repos/temporal",
            ])
            .current_dir(&parent)
            .output()
            .unwrap();

        let submodule = repos_dir.join("temporal");
        assert!(
            submodule.join(".git").is_file(),
            "Submodule should have .git file"
        );

        let paths = vec![repos_dir.clone()];
        let projects = available_projects_from_paths(&paths);

        assert_eq!(
            projects.get("temporal"),
            Some(&submodule),
            "Submodules should appear as projects (unlike worktrees)"
        );
    }

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/foo/bar");
        assert!(
            !expanded.to_string_lossy().contains('~'),
            "tilde should be expanded: {:?}",
            expanded
        );
        assert!(expanded.to_string_lossy().ends_with("/foo/bar"));

        assert_eq!(
            expand_tilde("/absolute/path"),
            PathBuf::from("/absolute/path")
        );
        assert_eq!(
            expand_tilde("relative/path"),
            PathBuf::from("relative/path")
        );
    }

    #[test]
    fn test_config_file_parse() {
        let toml_str = r#"
search_paths = [
    "~/src/repos",
    { path = "~/src", exclude = ["node_modules", "venv"] },
]
worktree_dir = "~/worktrees"
"#;
        let config: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(config.search_paths.len(), 2);
        assert_eq!(config.search_paths[0].path(), "~/src/repos");
        assert!(config.search_paths[0].exclude().is_empty());
        assert_eq!(config.search_paths[1].path(), "~/src");
        assert_eq!(config.search_paths[1].exclude(), &["node_modules", "venv"]);
        assert_eq!(config.worktree_dir.as_deref(), Some("~/worktrees"));
    }

    #[test]
    fn test_config_file_empty() {
        let config: ConfigFile = toml::from_str("").unwrap();
        assert!(config.search_paths.is_empty());
        assert!(config.worktree_dir.is_none());
    }
}
