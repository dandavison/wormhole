use std::path::PathBuf;
use std::thread;

use crate::project::StoreKey;
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
        let mut projects = projects::lock();
        let current_app = projects.current().map(|current| {
            let app = hammerspoon::current_application();
            projects.set_last_application(&current.store_key(), app.clone());
            app
        });
        let project = self.project.clone();
        let is_already_open = project.is_open();
        if !is_already_open {
            editor::open_workspace(&project);
        }
        let land_in = land_in.or_else(|| match mutation {
            Mutation::None | Mutation::RotateLeft | Mutation::RotateRight => {
                self.project.last_application.clone()
            }
            _ => parse_application(self.project.kv.get("land-in")).or({
                if is_already_open {
                    current_app
                } else {
                    None
                }
            }),
        });
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
                config::TERMINAL.focus();
                open_editor();
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
                thread::spawn(move || {
                    hammerspoon::launch_or_focus(config::editor().application_name())
                });
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
            if let Some(project) = projects.by_key(&StoreKey::project(repo)) {
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
        self.project.repo_path.join(
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
