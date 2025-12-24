use crate::editor::Editor;
use crate::terminal::Terminal;
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
