//! `GET /me/gliders` — every distinct wing the caller has flown, aggregated
//! from their `flights` rows over `(brand_id, kind, model_id)` (the composite
//! FK to `models`). Each row carries the canonical display info, the flight
//! count, and a `private` flag that's `true` when the model is pilot-private
//! (custom). Catalogue is small per pilot (typically <10 rows) so the whole
//! list ships in one response.
//!
//! No write routes: the pilot doesn't directly create wings here. Canonical
//! models come from the curated catalog; per-pilot customs are materialised by
//! the Leonardo importer (or, eventually, the upload UI when a pilot picks "new
//! wing"). When a wing's last referencing flight is deleted, the orphan-cleanup
//! trigger from migration 0013 reaps the custom `models` / `brands` rows
//! automatically.
//!
//! Mounted under the slide-aware tree, requires a session.

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::{AppError, AppState, auth::Identity};

pub fn router() -> Router<AppState> {
    Router::new().route("/me/gliders", get(list))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MyGliderDto {
    pub brand_id: String,
    pub brand_name: String,
    /// `glider_kind` as text (`pg`/`hg`/`sp`).
    pub kind: String,
    pub model_id: String,
    pub model_name: String,
    /// `glider_class` as text. `'unknown'` for customs the importer couldn't
    /// classify (HG-flex subtypes Leo doesn't disambiguate, SP).
    pub class: String,
    pub is_tandem: bool,
    /// `true` when the model row is pilot-private (`models.user_id IS NOT
    /// NULL`) — UI surfaces it as a "Custom" / "Private" badge.
    pub private: bool,
    /// Flights the caller has on this wing. Always ≥1 (rows with zero flights
    /// wouldn't show up in the GROUP BY).
    pub flights_count: i64,
}

async fn list(
    State(state): State<AppState>,
    identity: Identity,
) -> Result<Json<Vec<MyGliderDto>>, AppError> {
    let rows: Vec<MyGliderDto> = sqlx::query_as::<_, MyGliderDto>(
        "SELECT f.brand_id, \
                b.name           AS brand_name, \
                f.kind::text     AS kind, \
                f.model_id, \
                m.name           AS model_name, \
                m.class::text    AS class, \
                m.is_tandem      AS is_tandem, \
                (m.user_id IS NOT NULL) AS private, \
                COUNT(*)         AS flights_count \
           FROM flights f \
           JOIN brands b \
             ON b.id = f.brand_id \
           JOIN models m \
             ON m.brand_id = f.brand_id \
            AND m.kind     = f.kind \
            AND m.id       = f.model_id \
          WHERE f.user_id = $1 \
          GROUP BY f.brand_id, b.name, f.kind, f.model_id, m.name, m.class, \
                   m.is_tandem, m.user_id \
          ORDER BY f.kind, b.name, m.name",
    )
    .bind(identity.user_id)
    .fetch_all(state.pool())
    .await
    .map_err(into_internal)?;

    Ok(Json(rows))
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
