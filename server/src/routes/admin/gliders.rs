//! `GET /admin/gliders?kind=hg|pg|sp` — full glider catalog (brands +
//! models) for one kind, for the admin UI. The dataset per kind is small
//! (~hundreds of models) and rarely changes, so we ship the whole catalog in
//! one response and let the client build / filter the tree client-side.
//! Switching kinds in the UI triggers a fresh request — each kind is treated as
//! an independent source.
//!
//! Requires the `MANAGE_GLIDERS` bit.

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::{
    AppError, AppState,
    auth::{Identity, require_permission},
    user::Permissions,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/gliders", get(catalog))
}

#[derive(Debug, Deserialize)]
struct CatalogQuery {
    kind: String,
}

/// Kinds the admin dictionary covers. `'other'` is intentionally excluded — it
/// exists for unresolved uploads on the `gliders` row, not for the canonical
/// dictionary.
const ALLOWED_KINDS: [&str; 3] = ["hg", "pg", "sp"];

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Brand {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Model {
    pub brand_id: String,
    pub id: String,
    pub name: String,
    /// `glider_class` enum as text. Values are kind-compatible by the DB CHECK
    /// constraint, so the client can map directly off this without re-checking
    /// against `kind`.
    pub class: String,
    pub is_tandem: bool,
}

#[derive(Debug, Serialize)]
pub struct GliderCatalog {
    pub brands: Vec<Brand>,
    pub models: Vec<Model>,
}

async fn catalog(
    State(state): State<AppState>,
    identity: Identity,
    Query(q): Query<CatalogQuery>,
) -> Result<Json<GliderCatalog>, AppError> {
    require_permission(&identity, Permissions::MANAGE_GLIDERS)?;

    let kind = ALLOWED_KINDS
        .iter()
        .copied()
        .find(|k| *k == q.kind.as_str())
        .ok_or_else(|| {
            AppError::BadRequest(format!("kind must be one of: {}", ALLOWED_KINDS.join(", ")))
        })?;

    let pool = state.pool();

    let brands: Vec<Brand> = sqlx::query_as::<_, Brand>(
        "SELECT b.id, b.name \
         FROM brands b \
         WHERE EXISTS ( \
             SELECT 1 FROM glider_models m \
              WHERE m.brand_id = b.id AND m.kind = $1::glider_kind \
         ) \
         ORDER BY b.name",
    )
    .bind(kind)
    .fetch_all(pool)
    .await
    .map_err(into_internal)?;

    let models: Vec<Model> = sqlx::query_as::<_, Model>(
        "SELECT brand_id, id, name, class::text AS class, is_tandem \
         FROM glider_models \
         WHERE kind = $1::glider_kind \
         ORDER BY brand_id, name",
    )
    .bind(kind)
    .fetch_all(pool)
    .await
    .map_err(into_internal)?;

    Ok(Json(GliderCatalog { brands, models }))
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
