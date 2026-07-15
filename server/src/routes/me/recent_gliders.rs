//! `GET /me/gliders/recent` — the four most-recently-flown distinct wings for
//! the caller, deduped over `(brand_id, kind, model_id)` (the composite FK to
//! `models`) and ordered by the recency of each wing's latest flight. Feeds the
//! upload flow's "recent gliders" quick-pick.
//!
//! `takeoff_at` / `takeoff_timezone` / `launch_method` come from that latest
//! flight; `class` is the model's own class. Mounted under the slide-aware
//! tree, requires a session.

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::{AppError, AppState, auth::Identity};

pub fn router() -> Router<AppState> {
    Router::new().route("/me/gliders/recent", get(list))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RecentGliderDto {
    /// `glider_kind` as text (`pg`/`hg`/`sp`).
    pub kind: String,
    pub brand_id: String,
    pub brand_name: String,
    pub model_id: String,
    pub model_name: String,
    /// `glider_class` as text (e.g. `kingpost`/`topless` for HG, `en_b` for PG).
    pub class: String,
    /// Unix epoch seconds of the latest flight on this glider.
    pub takeoff_at: i64,
    /// IANA name (e.g. "Europe/Budapest") of that latest flight.
    pub takeoff_timezone: String,
    /// `launch_method` as text (`foot`/`winch`/`aerotow`) from that latest flight.
    pub launch_method: String,
}

async fn list(
    State(state): State<AppState>,
    identity: Identity,
) -> Result<Json<Vec<RecentGliderDto>>, AppError> {
    let rows: Vec<RecentGliderDto> = sqlx::query_as::<_, RecentGliderDto>(
        "SELECT g.brand_id, g.brand_name, g.kind, g.model_id, g.model_name, \
                g.class, g.takeoff_at, g.takeoff_timezone, g.launch_method \
           FROM ( \
             SELECT DISTINCT ON (f.brand_id, f.kind, f.model_id) \
                    f.brand_id, b.name AS brand_name, \
                    f.kind::text AS kind, \
                    f.model_id, m.name AS model_name, \
                    m.class::text AS class, \
                    EXTRACT(EPOCH FROM f.takeoff_at)::bigint AS takeoff_at, \
                    f.takeoff_timezone, \
                    f.launch_method::text AS launch_method \
               FROM flights f \
               JOIN brands b ON b.id = f.brand_id \
               JOIN models m ON m.brand_id = f.brand_id \
                            AND m.kind = f.kind \
                            AND m.id = f.model_id \
              WHERE f.user_id = $1 \
              ORDER BY f.brand_id, f.kind, f.model_id, f.takeoff_at DESC \
           ) g \
          ORDER BY g.takeoff_at DESC \
          LIMIT 4",
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
