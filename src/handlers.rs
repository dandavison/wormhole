use regex::Regex;
use std::{path::PathBuf, str};

use crate::{
    project::Project,
    project_path::ProjectPath,
    util::{info, warn},
    Application,
};

pub fn select_project_by_name(name: &str, land_in: Option<Application>) {
    info(&format!("select_project_by_name({name}, {land_in:?})"));
    if let Some(project) = Project::by_name(name) {
        project.open(land_in).unwrap();
    } else {
        warn(&format!("No matching project: {}", name));
    }
}

pub fn select_project_by_path(absolute_path: &str, land_in: Option<Application>) {
    let absolute_path = PathBuf::from(absolute_path);
    if let Some(project_path) = ProjectPath::from_absolute_path(&absolute_path) {
        project_path.open(land_in).unwrap();
    } else {
        warn(&format!(
            "Path doesn't correspond to a known project: {:?}",
            &absolute_path
        ))
    }
}

pub fn select_project_by_github_url(
    path: &str,
    line: Option<usize>,
    land_in: Option<Application>,
) -> Result<bool, String> {
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
            ProjectPath {
                project,
                relative_path: Some((path, line)),
            }
            .open(land_in)?;
            Ok(true)
        } else {
            Err(format!("No such repo: {}", repo))
        }
    } else {
        warn(&format!("Not a github URL: {}", path));
        Ok(false)
    }
}
