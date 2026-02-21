use std::io::{Read, Seek, SeekFrom};
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
    json_response(StatusCode::CREATED, &batch.to_response())
}

pub fn list_batches() -> Response<Body> {
    let store = batch::lock();
    let response = batch::BatchListResponse {
        batches: store.all().iter().map(|b| b.to_summary()).collect(),
    };
    json_response(StatusCode::OK, &response)
}

pub async fn batch_status(
    id: &str,
    req: &Request<Body>,
    completed: Option<usize>,
) -> Response<Body> {
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
                Some(Duration::from_secs(wait)),
            )
            .await;
        }
    }

    let store = batch::lock();
    match store.get(id) {
        Some(batch) => json_response(StatusCode::OK, &batch.to_response()),
        None => error_response(StatusCode::NOT_FOUND, "batch not found"),
    }
}

pub fn cancel(id: &str) -> Response<Body> {
    if batch::cancel_batch(id) {
        let store = batch::lock();
        match store.get(id) {
            Some(batch) => json_response(StatusCode::OK, &batch.to_response()),
            None => error_response(StatusCode::NOT_FOUND, "batch not found"),
        }
    } else {
        error_response(StatusCode::NOT_FOUND, "batch not found")
    }
}

pub fn batch_output(id: &str, run_idx: Option<usize>, offset: Option<u64>) -> Response<Body> {
    let run_idx = run_idx.unwrap_or(0);
    let offset = offset.unwrap_or(0);

    let store = batch::lock();
    let batch = match store.get(id) {
        Some(b) => b,
        None => return error_response(StatusCode::NOT_FOUND, "batch not found"),
    };
    let run = match batch.runs.get(run_idx) {
        Some(r) => r,
        None => return error_response(StatusCode::NOT_FOUND, "run not found"),
    };

    let done = matches!(
        run.status,
        batch::RunStatus::Succeeded | batch::RunStatus::Failed | batch::RunStatus::Cancelled
    );
    let stdout_path = run.stdout_path.clone();
    drop(store);

    let (content, new_offset) = read_from_offset(&stdout_path, offset);
    let json = serde_json::json!({
        "content": content,
        "offset": new_offset,
        "done": done,
    });
    json_response(StatusCode::OK, &json)
}

fn read_from_offset(path: &std::path::Path, offset: u64) -> (String, u64) {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return (String::new(), offset),
    };
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return (String::new(), offset);
    }
    let mut buf = Vec::new();
    let _ = file.read_to_end(&mut buf);
    let new_offset = offset + buf.len() as u64;
    (String::from_utf8_lossy(&buf).into_owned(), new_offset)
}

fn parse_prefer_wait(req: &Request<Body>) -> u64 {
    req.headers()
        .get("Prefer")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("wait="))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn json_response(status: StatusCode, value: &impl serde::Serialize) -> Response<Body> {
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
        .body(Body::from(serde_json::json!({ "error": msg }).to_string()))
        .unwrap()
}
