//! `GET /me/gliders/catalog?kind=hg|pg|sp` — brand + model catalog for one
//! kind, scoped to what the signed-in pilot may pick from: canonical rows
//! (`user_id IS NULL`) plus their own private customs (`user_id = caller`).
//!
//! Session-only sibling of the admin `GET /admin/gliders` (which is
//! `MANAGE_GLIDERS`-gated and canonical-only). Feeds the upload flow's glider
//! pickers, so a normal pilot needs it without the admin bit.

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState, auth::Identity, glider::CATALOG_KINDS};

pub fn router() -> Router<AppState> {
    Router::new().route("/me/gliders/catalog", get(catalog))
}

#[derive(Debug, Deserialize)]
struct CatalogQuery {
    kind: String,
}

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
    Query(query): Query<CatalogQuery>,
) -> Result<Json<GliderCatalog>, AppError> {
    let kind = CATALOG_KINDS
        .iter()
        .copied()
        .find(|k| *k == query.kind.as_str())
        .ok_or_else(|| {
            AppError::BadRequest(format!("kind must be one of: {}", CATALOG_KINDS.join(", ")))
        })?;

    let pool = state.pool();

    // Canonical (`user_id IS NULL`) plus the caller's own customs on both sides.
    let brands: Vec<Brand> = sqlx::query_as::<_, Brand>(
        "SELECT b.id, b.name \
         FROM brands b \
         WHERE (b.user_id IS NULL OR b.user_id = $2) \
           AND EXISTS ( \
               SELECT 1 FROM models m \
                WHERE m.brand_id = b.id \
                  AND m.kind = $1::glider_kind \
                  AND (m.user_id IS NULL OR m.user_id = $2) \
           ) \
         ORDER BY b.name",
    )
    .bind(kind)
    .bind(identity.user_id)
    .fetch_all(pool)
    .await
    .map_err(into_internal)?;

    let models: Vec<Model> = sqlx::query_as::<_, Model>(
        "SELECT brand_id, id, name, class::text AS class, is_tandem \
         FROM models \
         WHERE kind = $1::glider_kind AND (user_id IS NULL OR user_id = $2) \
         ORDER BY brand_id, name",
    )
    .bind(kind)
    .bind(identity.user_id)
    .fetch_all(pool)
    .await
    .map_err(into_internal)?;

    Ok(Json(GliderCatalog { brands, models }))
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
