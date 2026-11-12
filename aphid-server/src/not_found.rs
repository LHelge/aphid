use std::path::PathBuf;

use axum::{
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

/// The site's `404.html` (if any), served for every missing path.
///
/// Read per request rather than through `ServeFile` so the original request's
/// `Range` and conditional headers never apply to the error page.
#[derive(Debug, Clone)]
pub struct NotFoundPage {
    path: PathBuf,
}

impl NotFoundPage {
    pub fn new(root: &std::path::Path) -> Self {
        Self {
            path: root.join("404.html"),
        }
    }

    pub async fn handler(State(page): State<Self>) -> Response {
        match tokio::fs::read(&page.path).await {
            Ok(body) => (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body,
            )
                .into_response(),
            Err(_) => (StatusCode::NOT_FOUND, "404 Not Found\n").into_response(),
        }
    }
}
