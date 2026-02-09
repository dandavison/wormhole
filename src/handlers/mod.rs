pub mod dashboard;
pub mod describe;
pub mod project;

use hyper::{Body, Response};

pub fn favicon() -> Response<Body> {
    Response::builder()
        .header("Content-Type", "image/png")
        .body(Body::from(
            &include_bytes!("../../web/chrome-extension/icon48.png")[..],
        ))
        .unwrap()
}

pub fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

pub const WORMHOLE_RESPONSE_HTML: &str =
    "<html><body><script>window.close()</script>Sent into wormhole.</body></html>";
