use hyper::{Body, Response, StatusCode};
use std::collections::HashMap;

use crate::projects;

pub fn get_value(project_name: &str, key: &str) -> Response<Body> {
    let projects = projects::lock();

    if let Some(project) = projects.by_name(project_name) {
        if let Some(value) = project.kv.get(key) {
            Response::new(Body::from(value.clone()))
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

pub async fn set_value(project_name: &str, key: &str, body: Body) -> Response<Body> {
    let bytes = hyper::body::to_bytes(body).await.unwrap_or_default();
    let value = String::from_utf8_lossy(&bytes).to_string();

    let mut projects = projects::lock();

    if let Some(project_idx) = projects.all().iter().position(|p| p.name == project_name) {
        projects.all_mut()[project_idx]
            .kv
            .insert(key.to_string(), value.clone());
        save_kv_data(&projects);
        Response::new(Body::empty())
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", project_name)))
            .unwrap()
    }
}

pub fn delete_value(project_name: &str, key: &str) -> Response<Body> {
    let mut projects = projects::lock();

    if let Some(project_idx) = projects.all().iter().position(|p| p.name == project_name) {
        if projects.all_mut()[project_idx].kv.remove(key).is_some() {
            save_kv_data(&projects);
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

    if let Some(project) = projects.by_name(project_name) {
        let json = serde_json::to_string_pretty(&project.kv).unwrap();
        Response::new(Body::from(json))
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("Project '{}' not found", project_name)))
            .unwrap()
    }
}

pub fn get_all_kv() -> Response<Body> {
    let projects = projects::lock();
    let mut all_kv = HashMap::new();

    for project in projects.all() {
        if !project.kv.is_empty() {
            all_kv.insert(&project.name, &project.kv);
        }
    }

    let json = serde_json::to_string_pretty(&all_kv).unwrap();
    Response::new(Body::from(json))
}

fn save_kv_data(projects: &projects::Projects) {
    // For now, we'll save to a JSON file. In the future this could be SQLite or another storage backend
    use std::fs;
    use std::path::Path;

    let kv_file = Path::new("/tmp/wormhole-kv.json");
    let mut data = HashMap::new();

    for project in projects.all() {
        if !project.kv.is_empty() {
            data.insert(&project.name, &project.kv);
        }
    }

    let json = serde_json::to_string_pretty(&data).unwrap();
    fs::write(kv_file, json).unwrap_or_else(|e| {
        eprintln!("Failed to save KV data: {}", e);
    });
}

pub fn load_kv_data(projects: &mut projects::Projects) {
    use std::fs;
    use std::path::Path;

    let kv_file = Path::new("/tmp/wormhole-kv.json");
    if !kv_file.exists() {
        return;
    }

    let data = fs::read_to_string(kv_file).unwrap_or_default();
    if data.is_empty() {
        return;
    }

    let kv_data: HashMap<String, HashMap<String, String>> =
        serde_json::from_str(&data).unwrap_or_default();

    for project in projects.all_mut() {
        if let Some(kv) = kv_data.get(&project.name) {
            project.kv = kv.clone();
        }
    }
}
