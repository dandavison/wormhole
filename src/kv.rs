use hyper::{Body, Response, StatusCode};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config;
use crate::git;
use crate::project::{Project, StoreKey};
use crate::projects;

pub fn get_value(key: &StoreKey, kv_key: &str) -> Response<Body> {
    let projects = projects::lock();
    let Some(project) = projects.by_key(key) else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", key)))
            .unwrap();
    };
    match project.kv.get(kv_key) {
        Some(value) => Response::new(Body::from(value.clone())),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!(
                "Key '{}' not found in project '{}'",
                kv_key, key
            )))
            .unwrap(),
    }
}

pub async fn set_value(key: &StoreKey, kv_key: &str, body: Body) -> Response<Body> {
    let bytes = hyper::body::to_bytes(body).await.unwrap_or_default();
    let value = String::from_utf8_lossy(&bytes).to_string();

    let mut projects = projects::lock();

    if let Some(project) = projects.get_mut(key) {
        project.kv.insert(kv_key.to_string(), value);
        save_project_kv(project);
        Response::new(Body::empty())
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", key)))
            .unwrap()
    }
}

pub fn set_value_sync(key: &StoreKey, kv_key: &str, value: &str) {
    let mut projects = projects::lock();

    if let Some(project) = projects.get_mut(key) {
        project.kv.insert(kv_key.to_string(), value.to_string());
        save_project_kv(project);
    }
}

pub fn delete_value(key: &StoreKey, kv_key: &str) -> Response<Body> {
    let mut projects = projects::lock();

    if let Some(project) = projects.get_mut(key) {
        if project.kv.remove(kv_key).is_some() {
            save_project_kv(project);
            Response::new(Body::empty())
        } else {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!(
                    "Key '{}' not found in project '{}'",
                    kv_key, key
                )))
                .unwrap()
        }
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", key)))
            .unwrap()
    }
}

pub fn get_project_kv(key: &StoreKey) -> Response<Body> {
    let projects = projects::lock();
    match projects.by_key(key) {
        Some(project) => {
            let json = serde_json::to_string_pretty(&project.kv).unwrap();
            Response::new(Body::from(json))
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", key)))
            .unwrap(),
    }
}

fn wormhole_dir(project: &Project) -> PathBuf {
    crate::git::git_common_dir(&project.repo_path).join("wormhole")
}

fn kv_file(project: &Project) -> PathBuf {
    // Use store_key for filename to differentiate tasks from same repo
    // Replace : with _ in filename since : is not valid in filenames on some systems
    let filename = project.store_key().to_string().replace(':', "_");
    wormhole_dir(project)
        .join("kv")
        .join(format!("{}.json", filename))
}

fn save_project_kv(project: &Project) {
    use std::fs;

    if project.kv.is_empty() {
        let path = kv_file(project);
        let _ = fs::remove_file(path);
        return;
    }

    let path = kv_file(project);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let json = serde_json::to_string_pretty(&project.kv).unwrap();
    fs::write(&path, json).unwrap_or_else(|e| {
        eprintln!("Failed to save KV data for {}: {}", project.repo_name, e);
    });
}

pub fn load_kv_data(projects: &mut projects::Projects) {
    use std::fs;

    for project in projects.all_mut() {
        let path = kv_file(project);
        if !path.exists() {
            continue;
        }

        let data = match fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        if let Ok(kv) = serde_json::from_str::<HashMap<String, String>>(&data) {
            project.kv = kv;
        }
    }
}

/// List all KV data from disk for all discoverable projects and tasks.
/// Does not use cached data - reads directly from disk with concurrent I/O.
pub fn list_all_kv_fresh() -> Response<Body> {
    let available = config::available_projects();

    // Collect all (store_key, repo_path) pairs for projects and tasks
    let entries: Vec<(StoreKey, PathBuf)> = available
        .into_par_iter()
        .flat_map(|(name, path)| {
            let mut result = vec![];

            if !git::is_git_repo(&path) {
                return result;
            }

            // Add the main project
            result.push((StoreKey::project(name.clone()), path.clone()));

            // Discover tasks (worktrees)
            let worktrees_dir = git::worktree_base_path(&path);
            for wt in git::list_worktrees(&path) {
                if wt.path.starts_with(&worktrees_dir) {
                    if let Some(branch) = wt.branch {
                        result.push((StoreKey::task(name.clone(), branch), path.clone()));
                    }
                }
            }

            result
        })
        .collect();

    // Read KV files concurrently
    let all_kv: HashMap<String, HashMap<String, String>> = entries
        .into_par_iter()
        .filter_map(|(store_key, repo_path)| {
            let kv_path = kv_file_for_key(&store_key, &repo_path);
            let data = std::fs::read_to_string(&kv_path).ok()?;
            let kv: HashMap<String, String> = serde_json::from_str(&data).ok()?;
            if kv.is_empty() {
                None
            } else {
                Some((store_key.to_string(), kv))
            }
        })
        .collect();

    let json = serde_json::to_string_pretty(&all_kv).unwrap();
    Response::new(Body::from(json))
}

fn kv_file_for_key(key: &StoreKey, repo_path: &PathBuf) -> PathBuf {
    let filename = key.to_string().replace(':', "_");
    git::git_common_dir(repo_path)
        .join("wormhole")
        .join("kv")
        .join(format!("{}.json", filename))
}
