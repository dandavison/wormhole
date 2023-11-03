use crate::editor::Editor;
use crate::terminal::Terminal;

pub const EDITOR: Editor = Editor::VSCodeInsiders;
pub const TERMINAL: Terminal = Terminal::Alacritty { tmux: true };
pub const ENV_FILE: &'static str = "/tmp/wormhole.env";
pub const PROJECTS_FILE: &'static str = "~/.config/wormhole/wormhole-projects.txt";

// This port number is currently hardcoded in http clients such as the MacOS GUI
// app and the CLI utilities under cli/.
pub const WORMHOLE_PORT: u16 = 7117;

