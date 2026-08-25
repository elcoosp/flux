//! The HTTP asset server (FLUX-019).
//!
//! Serves files under the project root over plain HTTP on `:7332` (by default)
//! so the host app can fetch images and fonts referenced from `.flux` source.

use std::path::{Component, Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tokio::task::JoinHandle;

/// Spawns the asset server on `listener`, serving files under `root`.
pub(crate) fn spawn(listener: tokio::net::TcpListener, root: PathBuf) -> JoinHandle<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/assets/{*path}", get(serve_asset))
        .with_state(root);
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            tracing::warn!(%error, "asset server stopped");
        }
    })
}

/// Liveness probe used by the CLI and the integration tests.
async fn health() -> &'static str {
    "ok"
}

/// Serves one asset, rejecting any path that escapes the project root.
async fn serve_asset(State(root): State<PathBuf>, AxumPath(path): AxumPath<String>) -> Response {
    let Some(resolved) = resolve(&root, &path) else {
        return (
            StatusCode::BAD_REQUEST,
            format!("asset path `{path}` escapes the project root — hint: reference assets relative to the project root"),
        )
            .into_response();
    };
    match tokio::fs::read(&resolved).await {
        Ok(bytes) => (
            [(header::CONTENT_TYPE, content_type(&resolved))],
            Body::from(bytes),
        )
            .into_response(),
        Err(error) => (
            StatusCode::NOT_FOUND,
            format!(
                "asset `{path}` not found under {}: {error} — hint: check the file exists and is readable",
                root.display()
            ),
        )
            .into_response(),
    }
}

/// Joins `path` onto `root`, rejecting `..` traversal and absolute components.
fn resolve(root: &Path, path: &str) -> Option<PathBuf> {
    let candidate = Path::new(path);
    if candidate
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    Some(root.join(candidate))
}

/// Maps a file extension to a MIME type, defaulting to `application/octet-stream`.
fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("woff2") => "font/woff2",
        Some("flux" | "txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_is_rejected() {
        assert!(resolve(Path::new("/tmp/project"), "../secrets.txt").is_none());
    }

    #[test]
    fn nested_asset_resolves_under_root() {
        assert_eq!(
            resolve(Path::new("/tmp/project"), "img/logo.png"),
            Some(PathBuf::from("/tmp/project/img/logo.png"))
        );
    }

    #[test]
    fn content_type_is_derived_from_extension() {
        assert_eq!(content_type(Path::new("a/logo.png")), "image/png");
        assert_eq!(
            content_type(Path::new("a/blob.bin")),
            "application/octet-stream"
        );
    }
}
