use regex::Regex;
use std::{path::PathBuf, str};

use crate::{project::Project, project_path::ProjectPath};

pub fn select_project_by_name(path: &str) -> Result<bool, String> {
    if let Some(name) = path.strip_prefix("/project/") {
        println!("Handling as project: {}", name);
        if let Some(project) = Project::by_name(name) {
            project.open()?;
        }
    }
    Ok(false)
}

pub fn select_project_by_path(path: &str) -> Result<bool, String> {
    if let Some(absolute_path) = path.strip_prefix("/file/").map(PathBuf::from) {
        println!("Handling as path: {}", absolute_path.to_string_lossy());
        if let Some(project_path) = ProjectPath::from_absolute_path(absolute_path) {
            project_path.open()?;
            Ok(true)
        } else {
            Err(format!(
                "Path doesn't correspond to a known project: {}",
                path
            ))
        }
    } else {
        Ok(false)
    }
}

pub fn select_project_by_github_url(url: &str) -> Result<bool, String> {
    let re = Regex::new(r"/([^/]+)/([^/]+)/blob/([^/]+)/([^?]*)(?:\?line=(\d+))?").unwrap();
    if let Some(captures) = re.captures(url) {
        println!("Handling as github URL");
        let path = PathBuf::from(captures.get(4).unwrap().as_str());
        let line = captures
            .get(5)
            .map(|m| m.as_str().parse::<usize>().unwrap());
        let repo = captures.get(2).unwrap().as_str();

        println!(
            "path: {} line: {:?} repo: {}",
            path.to_string_lossy(),
            line,
            repo
        );
        if let Some(project) = Project::by_repo_name(repo) {
            ProjectPath {
                project,
                relative_path: path,
                line,
            }
            .open()?;
            Ok(true)
        } else {
            Err(format!("No such repo: {}", repo))
        }
    } else {
        eprintln!("Not a github URL: {}", url);
        Ok(false)
    }
}
