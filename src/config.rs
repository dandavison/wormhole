use crate::editor::Editor;
use crate::terminal::Terminal;

pub const EDITOR: Editor = Editor::VSCodeInsiders;
pub const TERMINAL: Terminal = Terminal::Alacritty { tmux: true };
pub const ENV_FILE: &'static str = "/tmp/wormhole.env";
pub const PROJECTS_FILE: &'static str = "~/.config/wormhole/wormhole-projects.yaml";
