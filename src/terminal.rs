use std::fs;

use crate::ps;
use crate::{config, hammerspoon, project::Project, tmux, util::warn, wezterm};

#[allow(dead_code)]
pub enum Terminal {
    Wezterm,
    Alacritty { tmux: bool },
}
use Terminal::*;

impl Terminal {
    pub fn open(&self, project: &Project) -> Result<(), String> {
        match self {
            Wezterm => wezterm::open(project),
            Alacritty { tmux: true } => tmux::open(project),
            _ => unimplemented!(),
        }
    }

    pub fn close(&self, project: &Project) {
        match self {
            Alacritty { tmux: true } => tmux::close(project),
            _ => unimplemented!(),
        }
    }

    pub fn focus(&self) {
        ps!("Focusing terminal");
        hammerspoon::launch_or_focus(self.application_name())
    }

    pub fn application_name(&self) -> &'static str {
        match self {
            Wezterm => "Wezterm",
            Alacritty { tmux: _ } => "Alacritty",
        }
    }
}

pub fn write_wormhole_env_vars(project: &Project) {
    if let Some(env_file) = config::ENV_FILE {
        fs::write(
            env_file,
            format!(
                "export WORMHOLE_PROJECT_NAME={} WORMHOLE_PROJECT_DIR={}",
                &project.name,
                project.path.as_path().to_str().unwrap()
            ),
        )
        .unwrap_or_else(|_| {
            warn(&format!(
                "Failed to write to config::ENV_FILE at {}",
                env_file
            ))
        })
    }
}
