use crate::editor::Editor;
use crate::terminal::Terminal;
use glob::Pattern;
use serde::Deserialize;
use std::sync::OnceLock;

pub const EDITOR: Editor = Editor::Cursor;
pub const TERMINAL: Terminal = Terminal::Alacritty { tmux: true };

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

// If you set this to Some(path) then project name and directory will be written
// to that file whenever wormhole changes project. This can be used for shell
// integration (e.g. prompt, cd-to-project-root).
pub const ENV_FILE: Option<&'static str> = Some("/tmp/wormhole.env");

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
    config()
        .available
        .exclude
        .iter()
        .any(|pattern| {
            Pattern::new(pattern)
                .map(|p| p.matches(name))
                .unwrap_or(false)
        })
}
