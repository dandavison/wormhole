pub mod batch;
pub mod dashboard;
pub mod describe;
pub mod doctor;
pub mod project;

use hyper::{Body, Response, StatusCode};

pub fn favicon() -> Response<Body> {
    Response::builder()
        .header("Content-Type", "image/png")
        .body(Body::from(
            &include_bytes!("../../web/chrome-extension/icon48.png")[..],
        ))
        .unwrap()
}

/// Serve a local file by absolute path (used for images in card.md markdown).
pub fn serve_asset(path: &str) -> Response<Body> {
    let absolute = format!("/{}", path);
    let path = std::path::Path::new(&absolute);
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    };
    match std::fs::read(path) {
        Ok(data) => Response::builder()
            .header("Content-Type", mime)
            .body(Body::from(data))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("File not found"))
            .unwrap(),
    }
}

pub fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

pub const WORMHOLE_RESPONSE_HTML: &str =
    "<html><body><script>window.close()</script>Sent into wormhole.</body></html>";
