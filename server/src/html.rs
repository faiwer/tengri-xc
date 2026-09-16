use anyhow::Context;
use axum::{
    extract::State,
    http::{Method, Uri, header},
    response::{Html, IntoResponse, Response},
};

use crate::{AppError, AppState};

/// Serves the SPA's `index.html` for client-side routes. Mounted as the root
/// router's fallback, so every request that isn't `/api/*` lands here.
pub(crate) async fn handler(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
) -> Result<Response, AppError> {
    // A guard, not routing: the reverse proxy decides what reaches us, but a
    // misconfigured one must not get HTML back for /assets/index-abc123.js.
    if (method != Method::GET && method != Method::HEAD) || has_extension(uri.path()) {
        return Err(AppError::NotFound);
    }

    // Concurrent misses each fetch and the last one wins; they all write the
    // same bytes, so single-flighting isn't worth a lock held across the await.
    let shell = match state.html_shell() {
        Some(shell) => shell,
        None => {
            let shell = fetch_shell(state.app_base_url()).await?;
            state.cache_html_shell(shell.clone());
            shell
        }
    };

    Ok(([(header::CACHE_CONTROL, "no-cache")], Html(shell)).into_response())
}

async fn fetch_shell(app_base_url: &str) -> anyhow::Result<String> {
    let url = format!("{app_base_url}/index.html");
    reqwest::get(&url)
        .await
        .and_then(reqwest::Response::error_for_status)
        .with_context(|| format!("fetching {url}"))?
        .text()
        .await
        .with_context(|| format!("reading {url}"))
}

fn has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|segment| segment.contains('.'))
}
