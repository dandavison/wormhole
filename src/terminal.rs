use std::fs;

use regex::Regex;

use crate::{config, git, hammerspoon, project::Project, tmux, util::warn, wezterm};

#[allow(dead_code)]
pub enum Terminal {
    Wezterm,
    Alacritty { tmux: bool },
}
use Terminal::*;

impl Terminal {
    pub fn exists(&self, project: &Project) -> bool {
        match self {
            Alacritty { tmux: true } => tmux::exists(project),
            _ => unimplemented!(),
        }
    }

    pub fn project_directories(&self) -> Vec<String> {
        match self {
            Alacritty { tmux: true } => tmux::project_directories(),
            _ => unimplemented!(),
        }
    }

    pub fn window_names(&self) -> Vec<String> {
        match self {
            Alacritty { tmux: true } => tmux::window_names(),
            _ => unimplemented!(),
        }
    }

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
        let jira_url = jira_url_for_name(&project.name).unwrap_or_default();
        let github_repo = project
            .github_repo
            .clone()
            .or_else(|| git::github_repo_from_remote(&project.path))
            .unwrap_or_default();
        let github_pr_url = if !github_repo.is_empty() {
            project
                .github_pr
                .or_else(|| crate::github::get_open_pr_number(project))
                .map(|pr| format!("https://github.com/{}/pull/{}", github_repo, pr))
                .unwrap_or_default()
        } else {
            String::new()
        };

        fs::write(
            env_file,
            format!(
                "export WORMHOLE_PROJECT_NAME='{}' WORMHOLE_PROJECT_DIR='{}' WORMHOLE_JIRA_URL='{}' WORMHOLE_GITHUB_REPO='{}' WORMHOLE_GITHUB_PR_URL='{}'",
                &project.name,
                project.path.as_path().to_str().unwrap(),
                jira_url,
                github_repo,
                github_pr_url,
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

fn jira_url_for_name(name: &str) -> Option<String> {
    let jira_key_re = Regex::new(r"^[A-Z]+-\d+").ok()?;
    if !jira_key_re.is_match(name) {
        return None;
    }
    let instance = std::env::var("JIRA_INSTANCE").ok()?;
    Some(format!("https://{}.atlassian.net/browse/{}", instance, name))
}
