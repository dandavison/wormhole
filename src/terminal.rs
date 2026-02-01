use regex::Regex;

use crate::{git, hammerspoon, project::Project, tmux, wezterm};

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
        if std::env::var("WORMHOLE_EDITOR").ok().as_deref() == Some("none") {
            return;
        }
        hammerspoon::launch_or_focus(self.application_name())
    }

    pub fn application_name(&self) -> &'static str {
        match self {
            Wezterm => "Wezterm",
            Alacritty { tmux: _ } => "Alacritty",
        }
    }
}

pub struct ShellEnvVars {
    pub project_name: String,
    pub project_dir: String,
    pub jira_url: String,
    pub github_repo: String,
    pub github_pr_url: String,
}

pub fn shell_env_vars(project: &Project) -> ShellEnvVars {
    let jira_url = jira_url_for_name(project.repo_name.as_str()).unwrap_or_default();
    let github_repo = project
        .github_repo
        .clone()
        .or_else(|| git::github_repo_from_remote(&project.repo_path))
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
    ShellEnvVars {
        project_name: project.repo_name.to_string(),
        project_dir: project.working_tree().to_string_lossy().to_string(),
        jira_url,
        github_repo,
        github_pr_url,
    }
}

pub fn shell_env_code(project: &Project) -> String {
    let vars = shell_env_vars(project);
    format!(
        "export WORMHOLE_PROJECT_NAME='{}' WORMHOLE_PROJECT_DIR='{}' WORMHOLE_JIRA_URL='{}' WORMHOLE_GITHUB_REPO='{}' WORMHOLE_GITHUB_PR_URL='{}'",
        vars.project_name,
        vars.project_dir,
        vars.jira_url,
        vars.github_repo,
        vars.github_pr_url,
    )
}

fn jira_url_for_name(name: &str) -> Option<String> {
    let jira_key_re = Regex::new(r"^([A-Z]+-\d+)").ok()?;
    let key = jira_key_re.captures(name)?.get(1)?.as_str();
    let instance = std::env::var("JIRA_INSTANCE").ok()?;
    Some(format!("https://{}.atlassian.net/browse/{}", instance, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jira_url_for_name_with_suffix() {
        std::env::set_var("JIRA_INSTANCE", "testinst");
        let url = jira_url_for_name("ACT-108-some-description");
        assert_eq!(
            url,
            Some("https://testinst.atlassian.net/browse/ACT-108".to_string())
        );
    }

    #[test]
    fn test_jira_url_for_name_bare_key() {
        std::env::set_var("JIRA_INSTANCE", "testinst");
        let url = jira_url_for_name("ACT-108");
        assert_eq!(
            url,
            Some("https://testinst.atlassian.net/browse/ACT-108".to_string())
        );
    }

    #[test]
    fn test_jira_url_for_name_not_jira() {
        std::env::set_var("JIRA_INSTANCE", "testinst");
        assert_eq!(jira_url_for_name("wormhole"), None);
        assert_eq!(jira_url_for_name("temporal"), None);
    }
}
