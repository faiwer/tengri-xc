//! Public site-settings reads. Mounted on the session-aware tree (no login
//! required, but the session layer is harmless on these routes). Anonymous
//! visitors call these on app boot and when navigating to `/terms` /
//! `/privacy`, so no `Identity` extraction.
//!
//! - `GET /site` — slim [`SiteDto`] with `site_name`, `can_register`, and
//!   `has_tos` / `has_privacy` booleans. Loaded once per app boot.
//! - `GET /site/tos`, `GET /site/privacy` — markdown body wrapped in `{ "md":
//!   "..." }` (200) or `404 Not Found` when the column is NULL. The body
//!   wrapper is JSON for consistency with the rest of the API surface (every
//!   other 2xx is JSON).

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::{
    AppError, AppState,
    site::{DocKind, SiteDto, fetch_site_doc, fetch_site_public},
};

pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/site", get(get_site))
        .route("/site/tos", get(get_tos))
        .route("/site/privacy", get(get_privacy))
}

#[derive(Debug, Serialize)]
struct SiteDocumentBody {
    md: String,
}

async fn get_site(State(state): State<AppState>) -> Result<Json<SiteDto>, AppError> {
    fetch_site_public(state.pool()).await.map(Json)
}

async fn get_tos(State(state): State<AppState>) -> Result<Json<SiteDocumentBody>, AppError> {
    get_doc(state, DocKind::Tos).await
}

async fn get_privacy(State(state): State<AppState>) -> Result<Json<SiteDocumentBody>, AppError> {
    get_doc(state, DocKind::Privacy).await
}

async fn get_doc(state: AppState, kind: DocKind) -> Result<Json<SiteDocumentBody>, AppError> {
    let md = fetch_site_doc(state.pool(), kind).await?;
    md.map(|md| Json(SiteDocumentBody { md }))
        .ok_or(AppError::NotFound)
}
