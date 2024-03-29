use std::path::{Path, PathBuf};
use std::thread;

use regex::Regex;

use crate::hammerspoon::current_application;
use crate::util::{info, warn};
use crate::wormhole::{Application, WindowAction};
use crate::{config, editor, project::Project};

#[derive(Clone, Debug)]
pub struct ProjectPath {
    pub project: Project,
    pub relative_path: Option<(PathBuf, Option<usize>)>,
}

impl ProjectPath {
    pub fn open(&self, land_in: Option<Application>) {
        info(&format!("ProjectPath({self:?}).open({land_in:?})"));
        let project = self.project.clone();
        let terminal_thread = thread::spawn(move || {
            config::TERMINAL.open(&project).unwrap_or_else(|err| {
                warn(&format!("Error opening {} in tmux: {}", &project.name, err))
            })
        });
        if self.project.is_terminal_only() {
            terminal_thread.join().unwrap();
            config::TERMINAL.focus();
            self.project.move_to_front();
            return;
        }
        let project_path = self.clone();
        let editor_window_action = match &land_in {
            Some(Application::Editor) => WindowAction::Focus,
            Some(Application::Terminal) => WindowAction::Raise,
            _ => match current_application() {
                Application::Editor => WindowAction::Focus,
                _ => WindowAction::Raise,
            },
        };
        let editor_thread = thread::spawn(move || {
            editor::open_path(&project_path, editor_window_action).unwrap_or_else(|err| {
                warn(&format!(
                    "Error opening {:?} in editor: {}",
                    project_path.relative_path, err
                ))
            });
        });
        terminal_thread.join().unwrap();
        editor_thread.join().unwrap();
        let flip_keybinding = Path::new("/tmp/wormhole-toggle").exists();
        let land_in_terminal = matches!(land_in, Some(Application::Terminal));
        if flip_keybinding ^ land_in_terminal {
            config::TERMINAL.focus()
        }
        self.project.move_to_front();
    }

    pub fn from_absolute_path(path: &Path) -> Option<Self> {
        if let Some(project) = Project::by_path(&path) {
            Some(ProjectPath {
                project: project.clone(),
                relative_path: Some((path.strip_prefix(&project.path).unwrap().into(), None)),
            })
        } else {
            warn(&format!(
                "Path doesn't correspond to a known project: {}",
                path.to_string_lossy()
            ));
            None
        }
    }

    pub fn from_github_url(path: &str, line: Option<usize>) -> Option<Self> {
        let re = Regex::new(r"/([^/]+)/([^/]+)/blob/([^/]+)/([^?]*)").unwrap();
        if let Some(captures) = re.captures(path) {
            info("Handling as github URL");
            let path = PathBuf::from(captures.get(4).unwrap().as_str());
            let repo = captures.get(2).unwrap().as_str();

            info(&format!(
                "path: {} line: {:?} repo: {}",
                path.to_string_lossy(),
                line,
                repo
            ));
            if let Some(project) = Project::by_repo_name(repo) {
                Some(ProjectPath {
                    project,
                    relative_path: Some((path, line)),
                })
            } else {
                warn(&format!("No such repo: {}", repo));
                None
            }
        } else {
            warn(&format!("Not a github URL: {}", path));
            None
        }
    }

    pub fn absolute_path(&self) -> PathBuf {
        self.project.path.join(
            self.relative_path
                .as_ref()
                .and_then(|(p, _)| p.to_str())
                .unwrap_or("".into()),
        )
    }
}
