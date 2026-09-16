//! Serves the SPA's `index.html` for client-side routes, with per-route
//! `<title>` and preview tags rendered in. The shell itself comes from the
//! static host (`APP_BASE_URL`) and is cached until `POST /api/html/reload`.

mod flight;
mod meta;
mod shell;

use std::sync::Arc;

use anyhow::Context;
use axum::{
    extract::State,
    http::{Method, StatusCode, Uri, header},
    response::{Html, IntoResponse, Response},
};

use crate::{
    AppError, AppState,
    site::{SiteMeta, fetch_site_meta},
};

pub(crate) use shell::HtmlShell;

/// Mounted as the root router's fallback, so every request that isn't `/api/*`
/// lands here.
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

    let shell = load_shell(&state).await?;
    let site = load_site_meta(&state).await?;
    let page = resolve(&state, uri.path(), &site).await?;
    let body = shell.render(&page.meta, &meta::image_url(state.app_base_url()));

    Ok((
        page.status,
        [(header::CACHE_CONTROL, "no-cache")],
        Html(body),
    )
        .into_response())
}

/// A resolved document: what to say about the page, and what to answer with.
struct Page {
    meta: meta::PageMeta,
    status: StatusCode,
}

impl Page {
    fn ok(meta: meta::PageMeta) -> Self {
        Self {
            meta,
            status: StatusCode::OK,
        }
    }

    fn not_found(meta: meta::PageMeta) -> Self {
        Self {
            meta,
            status: StatusCode::NOT_FOUND,
        }
    }
}

/// Routes with something of their own to say get a resolver; everything else
/// falls back to the site's name and description.
async fn resolve(state: &AppState, path: &str, site: &SiteMeta) -> Result<Page, AppError> {
    match path.strip_prefix("/flight/") {
        Some(id) if !id.is_empty() && !id.contains('/') => flight::resolve(state, id, site).await,
        _ => Ok(Page::ok(meta::PageMeta::site(site))),
    }
}

/// Concurrent misses each fetch and the last one wins; they all write the same
/// bytes, so single-flighting isn't worth a lock held across the await. Same
/// for [`load_site_meta`].
async fn load_shell(state: &AppState) -> Result<Arc<HtmlShell>, AppError> {
    if let Some(shell) = state.html_shell() {
        return Ok(shell);
    }

    let raw = fetch_shell(state.app_base_url()).await?;
    let shell = Arc::new(HtmlShell::parse(&raw)?);
    state.cache_html_shell(shell.clone());
    Ok(shell)
}

async fn load_site_meta(state: &AppState) -> Result<Arc<SiteMeta>, AppError> {
    if let Some(site) = state.site_meta() {
        return Ok(site);
    }

    let site = Arc::new(fetch_site_meta(state.pool()).await?);
    state.cache_site_meta(site.clone());
    Ok(site)
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
