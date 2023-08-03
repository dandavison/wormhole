use regex::Regex;
use std::{path::PathBuf, str};

use crate::{
    project::Project,
    project_path::ProjectPath,
    util::{info, warn},
};

pub enum Destination {
    VSCode,
    Tmux,
}

pub fn select_project_by_name(name: &str, query: Option<&str>) {
    if let Some(project) = Project::by_name(name) {
        let land_in = query
            .map(|s| match s.strip_prefix("land-in=") {
                Some("tmux") => Some(Destination::Tmux),
                Some("vscode") => Some(Destination::VSCode),
                _ => None,
            })
            .flatten();
        project.open(land_in).unwrap();
    } else {
        warn(&format!("No matching project: {}", name));
    }
}

pub fn select_project_by_path(absolute_path: &str) {
    let absolute_path = PathBuf::from(absolute_path);
    if let Some(project_path) = ProjectPath::from_absolute_path(&absolute_path) {
        project_path.open(None).unwrap();
    } else {
        warn(&format!(
            "Path doesn't correspond to a known project: {:?}",
            &absolute_path
        ))
    }
}

pub fn select_project_by_github_url(path: &str, query: Option<&str>) -> Result<bool, String> {
    let re = Regex::new(r"/([^/]+)/([^/]+)/blob/([^/]+)/([^?]*)(?:\?line=(\d+))?").unwrap();
    if let Some(captures) = re.captures(path) {
        info("Handling as github URL");
        let path = PathBuf::from(captures.get(4).unwrap().as_str());
        let line = query
            .and_then(|s| s.strip_prefix("line="))
            .and_then(|s| s.parse::<usize>().ok());
        let repo = captures.get(2).unwrap().as_str();

        info(&format!(
            "path: {} line: {:?} repo: {}",
            path.to_string_lossy(),
            line,
            repo
        ));
        if let Some(project) = Project::by_repo_name(repo) {
            ProjectPath {
                project,
                relative_path: path,
                line,
            }
            .open(None)?;
            Ok(true)
        } else {
            Err(format!("No such repo: {}", repo))
        }
    } else {
        warn(&format!("Not a github URL: {}", path));
        Ok(false)
    }
}
