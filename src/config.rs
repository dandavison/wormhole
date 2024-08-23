use crate::editor::Editor;
use crate::terminal::Terminal;

pub const EDITOR: Editor = Editor::Cursor;
pub const TERMINAL: Terminal = Terminal::Alacritty { tmux: true };

pub const PROJECTS_FILE: &'static str = "~/.config/wormhole/wormhole-projects.txt";

// This port number is currently hardcoded in http clients such as the MacOS GUI
// app and the CLI utilities under cli/.
pub const WORMHOLE_PORT: u16 = 7117;

// If you set this to Some(path) then project name and directory will be written
// to that file whenever wormhole changes project. This can be used for shell
// integration (e.g. prompt, cd-to-project-root).
pub const ENV_FILE: Option<&'static str> = Some("/tmp/wormhole.env");
