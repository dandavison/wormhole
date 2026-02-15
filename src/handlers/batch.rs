use std::time::Duration;

use hyper::{Body, Request, Response, StatusCode};

use crate::batch;
use crate::handlers::project::poll_until;

pub async fn start_batch(req: Request<Body>) -> Response<Body> {
    let body = match hyper::body::to_bytes(req.into_body()).await {
        Ok(b) => b,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };
    let request: batch::BatchRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };
    if request.command.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "command must not be empty");
    }
    if request.runs.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "runs must not be empty");
    }

    let id = batch::create_batch(request);
    batch::spawn_batch(&id);

    let store = batch::lock();
    let batch = store.get(&id).unwrap();
    json_response(StatusCode::CREATED, &batch_to_json(batch))
}

pub fn list_batches() -> Response<Body> {
    let store = batch::lock();
    let batches: Vec<serde_json::Value> = store
        .all()
        .iter()
        .map(|b| {
            serde_json::json!({
                "id": b.id,
                "command": b.command,
                "created_at": system_time_to_epoch(b.created_at),
                "total": b.runs.len(),
                "completed": b.completed_count(),
                "done": b.is_done(),
            })
        })
        .collect();
    json_response(StatusCode::OK, &serde_json::json!({ "batches": batches }))
}

pub async fn batch_status(id: &str, req: &Request<Body>, completed: Option<usize>) -> Response<Body> {
    let wait = parse_prefer_wait(req);

    if wait > 0 {
        if let Some(client_completed) = completed {
            let id_owned = id.to_string();
            poll_until(
                move || {
                    let store = batch::lock();
                    match store.get(&id_owned) {
                        Some(b) => b.completed_count() > client_completed,
                        None => true,
                    }
                },
                batch::subscribe(),
                Duration::from_secs(wait),
            )
            .await;
        }
    }

    let store = batch::lock();
    match store.get(id) {
        Some(batch) => json_response(StatusCode::OK, &batch_to_json(batch)),
        None => error_response(StatusCode::NOT_FOUND, "batch not found"),
    }
}

pub fn cancel(id: &str) -> Response<Body> {
    if batch::cancel_batch(id) {
        let store = batch::lock();
        match store.get(id) {
            Some(batch) => json_response(StatusCode::OK, &batch_to_json(batch)),
            None => error_response(StatusCode::NOT_FOUND, "batch not found"),
        }
    } else {
        error_response(StatusCode::NOT_FOUND, "batch not found")
    }
}

fn batch_to_json(batch: &batch::Batch) -> serde_json::Value {
    let runs: Vec<serde_json::Value> = batch
        .runs
        .iter()
        .map(|r| {
            let mut obj = serde_json::json!({
                "key": r.key,
                "dir": r.dir,
                "status": r.status,
            });
            if let Some(code) = r.exit_code {
                obj["exit_code"] = serde_json::json!(code);
            }
            if let Some(t) = r.started_at {
                obj["started_at"] = serde_json::json!(system_time_to_epoch(t));
            }
            if let Some(t) = r.finished_at {
                obj["finished_at"] = serde_json::json!(system_time_to_epoch(t));
            }
            obj
        })
        .collect();

    serde_json::json!({
        "id": batch.id,
        "command": batch.command,
        "created_at": system_time_to_epoch(batch.created_at),
        "total": batch.runs.len(),
        "completed": batch.completed_count(),
        "done": batch.is_done(),
        "runs": runs,
    })
}

fn system_time_to_epoch(t: std::time::SystemTime) -> f64 {
    t.duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn parse_prefer_wait(req: &Request<Body>) -> u64 {
    req.headers()
        .get("Prefer")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("wait="))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn json_response(status: StatusCode, value: &serde_json::Value) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string_pretty(value).unwrap()))
        .unwrap()
}

fn error_response(status: StatusCode, msg: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({ "error": msg }).to_string(),
        ))
        .unwrap()
}
