use std::path::PathBuf;
use std::thread;

use crate::project::ProjectKey;
use crate::projects::{self, Mutation, Projects};
use crate::util::warn;
use crate::wormhole::Application;
use crate::{config, editor, hammerspoon, project::Project};
use crate::{ps, util};
use regex::Regex;

#[derive(Clone, Debug)]
pub struct ProjectPath {
    pub project: Project,
    pub relative_path: Option<(PathBuf, Option<usize>)>,
}

impl ProjectPath {
    pub fn open(&self, mutation: Mutation, land_in: Option<Application>) {
        self.open_with_options(mutation, land_in, false);
    }

    pub fn open_with_options(
        &self,
        mutation: Mutation,
        land_in: Option<Application>,
        skip_editor: bool,
    ) {
        let mut projects = projects::lock();
        let current_app = if projects.current().is_some() {
            Some(hammerspoon::current_application())
        } else {
            None
        };
        let project = self.project.clone();
        let is_already_open = project.is_open();
        if !is_already_open && !skip_editor {
            editor::open_workspace(&project);
        }
        let land_in = if skip_editor {
            Some(Application::Terminal)
        } else {
            // navigate() pre-rotates the ring then calls us with Mutation::None;
            // only inherit focus from the current app for explicit opens.
            let is_explicit_switch = !matches!(mutation, Mutation::None);
            land_in
                .or_else(|| parse_application(self.project.kv.get("land-in")))
                .or_else(|| {
                    if is_already_open && is_explicit_switch {
                        current_app
                    } else {
                        None
                    }
                })
        };
        projects.apply(mutation, &self.project.store_key());
        if util::debug() {
            projects.print();
        }
        drop(projects);
        let open_terminal = move || {
            config::TERMINAL.open(&project).unwrap_or_else(|err| {
                warn(&format!(
                    "Error opening {} in terminal: {}",
                    &project.repo_name, err
                ))
            })
        };

        if skip_editor {
            open_terminal();
            config::TERMINAL.focus();
            return;
        }

        let project_path = self.clone();
        let open_editor = move || {
            editor::open_path(&project_path).unwrap_or_else(|err| {
                warn(&format!(
                    "Error opening {:?} in editor: {}",
                    project_path.relative_path, err
                ))
            });
        };

        match &land_in {
            Some(Application::Terminal) => {
                open_terminal();
                open_editor();
                config::TERMINAL.focus();
            }
            Some(Application::Editor) => {
                open_editor();
                config::editor().focus();
                open_terminal();
            }
            None => {
                let terminal_thread = thread::spawn(open_terminal);
                let editor_thread = thread::spawn(open_editor);
                terminal_thread.join().unwrap();
                editor_thread.join().unwrap();
                thread::sleep(std::time::Duration::from_millis(100));
                config::editor().focus();
            }
        }
    }

    pub fn from_absolute_path(path: &str, projects: &Projects) -> Option<Self> {
        let re = Regex::new(r"^(.*):([^:]*)$").unwrap();
        let (path, line) = if let Some(captures) = re.captures(path) {
            let line = captures.get(2).unwrap().as_str().parse::<usize>().ok();
            (PathBuf::from(captures.get(1).unwrap().as_str()), line)
        } else {
            (PathBuf::from(path), None)
        };
        if let Some(project) = projects.by_path(&path) {
            Some(ProjectPath {
                project: project.clone(),
                relative_path: Some((path, line)),
            })
        } else {
            let keys: Vec<_> = projects.keys().iter().map(|k| k.to_string()).collect();
            warn(&format!(
                "Path {} doesn't correspond to a project.\n Projects are {}",
                path.to_string_lossy(),
                keys.join(", ")
            ));
            None
        }
    }

    pub fn from_github_url(path: &str, line: Option<usize>, projects: &Projects) -> Option<Self> {
        let re = Regex::new(r"/([^/]+)/([^/]+)/blob/([^/]+)/([^?]*)").unwrap();
        if let Some(captures) = re.captures(path) {
            ps!("Handling as github URL");
            let path = PathBuf::from(captures.get(4).unwrap().as_str());
            let repo = captures.get(2).unwrap().as_str();

            ps!(
                "path: {} line: {:?} repo: {}",
                path.to_string_lossy(),
                line,
                repo
            );
            if let Some(project) = projects.by_key(&ProjectKey::project(repo)) {
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
        let base = self
            .project
            .worktree_path()
            .unwrap_or_else(|| self.project.repo_path.clone());
        base.join(
            self.relative_path
                .as_ref()
                .and_then(|(p, _)| p.to_str())
                .unwrap_or(""),
        )
    }
}

fn parse_application(s: Option<&String>) -> Option<Application> {
    s.and_then(|v| match v.as_str() {
        "terminal" => Some(Application::Terminal),
        "editor" => Some(Application::Editor),
        _ => None,
    })
}
