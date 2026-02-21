use hyper::{Body, Request, Response, StatusCode};
use std::time::Duration;

use crate::handlers::project::poll_until;
use crate::messages::{self, PublishRequest};

pub async fn poll(name: &str, role: &str, wait_secs: u64) -> Response<Body> {
    let id = messages::lock().register(name, role);

    if messages::lock().has_messages(id) {
        let msgs = messages::lock().drain(id);
        messages::lock().unregister(id);
        return json_response(&msgs);
    }

    let rx = messages::subscribe();
    poll_until(
        || messages::lock().has_messages(id),
        rx,
        Duration::from_secs(wait_secs),
    )
    .await;

    let msgs = messages::lock().drain(id);
    messages::lock().unregister(id);
    json_response(&msgs)
}

pub async fn publish(name: &str, req: Request<Body>) -> Response<Body> {
    let body = match hyper::body::to_bytes(req.into_body()).await {
        Ok(b) => b,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Failed to read body"))
                .unwrap()
        }
    };
    let publish_req: PublishRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Invalid JSON: {e}")))
                .unwrap()
        }
    };
    let (target, notification) = publish_req.into_parts();
    messages::lock().publish(name, &target, notification);
    Response::new(Body::from(""))
}

fn json_response<T: serde::Serialize>(value: &T) -> Response<Body> {
    Response::builder()
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(value).unwrap()))
        .unwrap()
}
