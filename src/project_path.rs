use std::path::{Path, PathBuf};
use std::thread;

use crate::hammerspoon::current_application;
use crate::util::{info, warn};
use crate::{editor, project::Project, tmux};
use crate::{Application, WindowAction};

#[derive(Clone, Debug)]
pub struct ProjectPath {
    pub project: Project,
    pub relative_path: Option<(PathBuf, Option<usize>)>,
}

impl ProjectPath {
    pub fn open(&self, land_in: Option<Application>) -> Result<bool, String> {
        info(&format!("ProjectPath({self:?}).open({land_in:?})"));
        let project = self.project.clone();
        let terminal_thread = thread::spawn(move || {
            tmux::open(&project).unwrap_or_else(|err| {
                warn(&format!("Error opening {} in tmux: {}", &project.name, err))
            })
        });
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
            tmux::focus()
        }
        self.project.move_to_front();
        Ok(true)
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

    pub fn absolute_path(&self) -> PathBuf {
        self.project.path.join(
            self.relative_path
                .as_ref()
                .and_then(|(p, _)| p.to_str())
                .unwrap_or("".into()),
        )
    }
}
