use std::path::PathBuf;
use std::thread;

use crate::projects::{self, Mutation, Projects};
use crate::util::warn;
use crate::wormhole::Application;
use crate::{config, editor, project::Project};
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
        let project = self.project.clone();

        if !project.is_open() {
            editor::open_workspace(&project);
        }

        let terminal_thread = thread::spawn(move || {
            config::TERMINAL.open(&project).unwrap_or_else(|err| {
                warn(&format!(
                    "Error opening {} in terminal: {}",
                    &project.name, err
                ))
            })
        });
        if self.project.is_terminal_only() {
            terminal_thread.join().unwrap();
            config::TERMINAL.focus();
            projects.move_to_front(&self.project.name);
            return;
        }
        let project_path = self.clone();
        let editor_thread = thread::spawn(move || {
            editor::open_path(&project_path).unwrap_or_else(|err| {
                warn(&format!(
                    "Error opening {:?} in editor: {}",
                    project_path.relative_path, err
                ))
            });
        });
        terminal_thread.join().unwrap();
        editor_thread.join().unwrap();
        // The editor has focus; take it back if necessary
        if matches!(land_in, Some(Application::Terminal)) {
            config::TERMINAL.focus()
        }
        projects.apply(mutation, &self.project.name);
        if util::debug() {
            projects.print();
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
            warn(&format!(
                "Path {} doesn't correspond to a project.\n Projects are {}",
                path.to_string_lossy(),
                projects.names().join(", ")
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
            if let Some(project) = projects.by_name(repo) {
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
