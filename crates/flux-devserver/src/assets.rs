//! The HTTP asset server (FLUX-019, FLUX-019b / PE-D).
//!
//! Serves files under the project root over plain HTTP on `:7332` (by default)
//! so the host app can fetch images and fonts referenced from `.flux` source.
//! Asset responses carry a one-year `immutable` `Cache-Control` directive and an
//! `ETag` derived from the file's modification time and length, so the host can
//! issue conditional requests and skip re-downloading unchanged assets.

use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tokio::task::JoinHandle;

/// One year in seconds — assets are content-addressed by path and never
/// mutated in place, so a long `max-age` with `immutable` is safe.
const CACHE_MAX_AGE_SECS: u64 = 31_536_000;

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
///
/// Successful responses carry `Cache-Control: max-age=31536000, immutable` and an
/// `ETag`. When the request's `If-None-Match` matches the current `ETag`, a `304
/// Not Modified` is returned with no body.
async fn serve_asset(
    State(root): State<PathBuf>,
    AxumPath(path): AxumPath<String>,
    request_headers: HeaderMap,
) -> Response {
    let Some(resolved) = resolve(&root, &path) else {
        return (
            StatusCode::BAD_REQUEST,
            format!(
                "asset path `{path}` escapes the project root — hint: reference assets relative to the project root"
            ),
        )
            .into_response();
    };

    let Ok(metadata) = tokio::fs::metadata(&resolved).await else {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "asset `{path}` not found under {} — hint: check the file exists and is readable",
                root.display()
            ),
        )
            .into_response();
    };

    let etag = compute_etag(&metadata);

    if let Some(if_none_match) = request_headers.get(header::IF_NONE_MATCH) {
        if if_none_match.as_bytes() == etag.as_bytes() {
            return StatusCode::NOT_MODIFIED.into_response();
        }
    }

    match tokio::fs::read(&resolved).await {
        Ok(bytes) => {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type(&resolved)),
            );
            // `max-age=N, immutable` is pure ASCII, so `from_str` cannot fail.
            let cache_control = HeaderValue::from_str(&format!("max-age={CACHE_MAX_AGE_SECS}, immutable"))
                .unwrap_or_else(|_| HeaderValue::from_static("max-age=31536000, immutable"));
            response_headers.insert(header::CACHE_CONTROL, cache_control);
            // `etag` is composed solely of ASCII hex digits and double quotes
            // (see `compute_etag`), which are always valid header octets.
            let etag_value = HeaderValue::from_str(&etag)
                .unwrap_or_else(|_| HeaderValue::from_static("\"0\""));
            response_headers.insert(header::ETAG, etag_value);
            (StatusCode::OK, response_headers, Body::from(bytes)).into_response()
        }
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

/// Derives a strong `ETag` from the file's length and modification time.
///
/// The result is wrapped in double quotes per RFC 7232 and contains only ASCII
/// hex digits plus the surrounding quotes, so it is always a valid header value.
fn compute_etag(metadata: &std::fs::Metadata) -> String {
    let len = metadata.len();
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
    match modified {
        Some(duration) => format!("\"{len:x}-{:x}\"", duration.as_nanos()),
        None => format!("\"{len:x}\""),
    }
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
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Returns a fresh, unique temporary directory for asset-server tests.
    fn temp_asset_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("flux-asset-cache-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("test fixture dir must be creatable");
        dir
    }

    #[test]
    fn traversal_is_rejected() {
        assert!(resolve(Path::new("/tmp/project"), "../secrets.txt").is_none());
    }

    #[tokio::test]
    async fn traversal_request_returns_rejected_status() {
        // A `../../etc/passwd`-style request must be refused: the server must not
        // serve files outside the project root. The traversal guard rejects before
        // any filesystem read, returning 400 with an explanatory body.
        let response = serve_asset(
            State(PathBuf::from("/tmp/project")),
            AxumPath("../../etc/passwd".to_string()),
            HeaderMap::new(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body is readable");
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains("escapes the project root"),
            "traversal rejection must explain the guard: {text}"
        );
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

    #[tokio::test]
    async fn cached_response_carries_cache_control_and_etag() {
        let dir = temp_asset_dir();
        tokio::fs::write(dir.join("logo.png"), b"fake-image-bytes")
            .await
            .expect("test asset must be writable");

        let response = serve_asset(
            State(dir.clone()),
            AxumPath("logo.png".to_string()),
            HeaderMap::new(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let cache_control = response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("cache-control header must be present");
        let cache_text = cache_control.to_str().expect("cache-control is ASCII");
        assert!(
            cache_text.contains("max-age="),
            "expected max-age directive, got {cache_text}"
        );
        assert!(
            cache_text.contains("immutable"),
            "expected immutable directive, got {cache_text}"
        );
        let etag = response
            .headers()
            .get(header::ETAG)
            .expect("etag header must be present");
        assert!(!etag.is_empty(), "etag must be non-empty");
    }

    #[tokio::test]
    async fn if_none_match_returns_304() {
        let dir = temp_asset_dir();
        tokio::fs::write(dir.join("logo.png"), b"fake-image-bytes")
            .await
            .expect("test asset must be writable");

        let first = serve_asset(
            State(dir.clone()),
            AxumPath("logo.png".to_string()),
            HeaderMap::new(),
        )
        .await;
        let etag = first
            .headers()
            .get(header::ETAG)
            .expect("first response must carry an etag")
            .to_str()
            .expect("etag is ASCII")
            .to_string();

        let mut request_headers = HeaderMap::new();
        request_headers.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(&etag).expect("etag is a valid header value"),
        );

        let second = serve_asset(
            State(dir),
            AxumPath("logo.png".to_string()),
            request_headers,
        )
        .await;

        assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    }
}
