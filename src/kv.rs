use hyper::{Body, Response, StatusCode};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::project::Project;
use crate::projects;

pub fn get_value(project_name: &str, key: &str) -> Response<Body> {
    let projects = projects::lock();
    let Some(project) = projects.resolve(project_name) else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", project_name)))
            .unwrap();
    };
    match project.kv.get(key) {
        Some(value) => Response::new(Body::from(value.clone())),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!(
                "Key '{}' not found in project '{}'",
                key, project_name
            )))
            .unwrap(),
    }
}

pub async fn set_value(project_name: &str, key: &str, body: Body) -> Response<Body> {
    let bytes = hyper::body::to_bytes(body).await.unwrap_or_default();
    let value = String::from_utf8_lossy(&bytes).to_string();

    let mut projects = projects::lock();

    if let Some(project) = projects.get_mut(project_name) {
        project.kv.insert(key.to_string(), value);
        save_project_kv(project);
        Response::new(Body::empty())
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", project_name)))
            .unwrap()
    }
}

pub fn set_value_sync(project_name: &str, key: &str, value: &str) {
    let mut projects = projects::lock();

    if let Some(project) = projects.get_mut(project_name) {
        project.kv.insert(key.to_string(), value.to_string());
        save_project_kv(project);
    }
}

pub fn delete_value(project_name: &str, key: &str) -> Response<Body> {
    let mut projects = projects::lock();

    if let Some(project) = projects.get_mut(project_name) {
        if project.kv.remove(key).is_some() {
            save_project_kv(project);
            Response::new(Body::empty())
        } else {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!(
                    "Key '{}' not found in project '{}'",
                    key, project_name
                )))
                .unwrap()
        }
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", project_name)))
            .unwrap()
    }
}

pub fn get_project_kv(project_name: &str) -> Response<Body> {
    let projects = projects::lock();
    match projects.resolve(project_name) {
        Some(project) => {
            let json = serde_json::to_string_pretty(&project.kv).unwrap();
            Response::new(Body::from(json))
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", project_name)))
            .unwrap(),
    }
}

pub fn get_all_kv() -> Response<Body> {
    let projects = projects::lock();
    let mut all_kv = HashMap::new();

    for project in projects.all() {
        if !project.kv.is_empty() {
            all_kv.insert(&project.repo_name, &project.kv);
        }
    }

    let json = serde_json::to_string_pretty(&all_kv).unwrap();
    Response::new(Body::from(json))
}

fn wormhole_dir(project: &Project) -> PathBuf {
    crate::git::git_common_dir(&project.repo_path).join("wormhole")
}

fn kv_file(project: &Project) -> PathBuf {
    wormhole_dir(project)
        .join("kv")
        .join(format!("{}.json", project.repo_name))
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
