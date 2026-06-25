//! Embedded web UI assets and the SPA-serving fallback.
//!
//! The built frontend in `web/dist` is baked into the binary at compile time
//! (the same approach as `sqlx::migrate!` embedding migrations), so the single
//! binary serves both the API and the UI. A fresh checkout only contains
//! `web/dist/.gitkeep`; until `npm --prefix web run build` runs, the fallback
//! reports "frontend not built" instead of serving a page.

use axum::{
    body::Body,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;
use tracing::error;

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct Assets;

/// Serve an embedded asset, falling back to `index.html` for unknown paths so
/// client-side deep links resolve to the SPA shell. Returns `404` when no
/// frontend has been built into the binary.
pub async fn spa_fallback(uri: Uri) -> Response {
    let requested = uri.path().trim_start_matches('/');
    let requested = if requested.is_empty() {
        "index.html"
    } else {
        requested
    };

    if let Some(file) = Assets::get(requested) {
        return serve(requested, file.data.into_owned());
    }
    match Assets::get("index.html") {
        Some(index) => serve("index.html", index.data.into_owned()),
        None => (StatusCode::NOT_FOUND, "frontend not built").into_response(),
    }
}

fn serve(path: &str, bytes: Vec<u8>) -> Response {
    let content_type = content_type(path);
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, cache_control(path))
        .body(Body::from(bytes));

    match response {
        Ok(response) => response,
        Err(error) => {
            error!(error = %error, path, "failed to build asset response");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn content_type(path: &str) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_owned()
}

/// Content-hashed assets under `assets/` can be cached forever; everything else
/// (notably `index.html`) must be revalidated so deploys are picked up.
fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_maps_known_extensions() {
        assert_eq!(content_type("index.html"), "text/html");
        assert_eq!(content_type("assets/app.css"), "text/css");
        assert!(content_type("assets/app.js").contains("javascript"));
    }

    #[test]
    fn content_type_falls_back_to_octet_stream() {
        assert_eq!(content_type("weird.unknownext"), "application/octet-stream");
    }

    #[test]
    fn hashed_assets_are_immutable_others_revalidate() {
        assert_eq!(
            cache_control("assets/app-abc123.js"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("favicon.svg"), "no-cache");
    }

    /// Deep links resolve to the SPA shell when a frontend is embedded, or a
    /// clean 404 when it is not. Robust whether or not `web/dist` was built.
    #[tokio::test]
    async fn deep_link_serves_index_or_reports_not_built() {
        let response = spa_fallback("/some/deep/link".parse().unwrap()).await;
        if Assets::get("index.html").is_some() {
            assert_eq!(response.status(), StatusCode::OK);
        } else {
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }
}
