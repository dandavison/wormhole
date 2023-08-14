use crate::{hammerspoon, project::Project, tmux, util::info};

pub enum Terminal {
    Alacritty { tmux: bool },
}
use Terminal::*;

impl Terminal {
    pub fn open(&self, project: &Project) -> Result<(), String> {
        match self {
            Alacritty { tmux: true } => tmux::open(project),
            _ => unimplemented!(),
        }
    }

    pub fn focus(&self) {
        info("Focusing terminal");
        hammerspoon::launch_or_focus(self.application_name())
    }

    fn application_name(&self) -> &'static str {
        match self {
            Alacritty { tmux: _ } => "Alacritty",
        }
    }
}
