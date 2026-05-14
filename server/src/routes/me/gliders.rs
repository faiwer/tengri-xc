//! `/me/gliders/*` — the signed-in pilot's own wings.
//!
//! - `GET    /me/gliders`        — every glider owned by the caller, with the
//!   number of flights that reference it. The catalog is small per pilot
//!   (typically <10 rows) so the whole list ships in one response and the
//!   client groups it by kind / brand client-side.
//! - `DELETE /me/gliders/:id`    — drop a glider. Refuses with 409 when any
//!   flight still references it; the FE disables the button in that case too,
//!   but the server enforces it independently. 404 when the row doesn't exist
//!   *or* belongs to someone else (don't leak ownership).
//!
//! Mounted under the slide-aware tree, requires a session.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::Serialize;

use crate::{AppError, AppState, auth::Identity};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me/gliders", get(list))
        .route("/me/gliders/{id}", axum::routing::delete(delete))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MyGliderDto {
    pub id: i32,
    /// `glider_kind` enum as text (`pg`/`hg`/`sp`/`other`).
    pub kind: String,
    /// `glider_class` enum as text. NULL for `kind = 'other'` and for
    /// brand-only Leonardo imports where the model didn't resolve.
    pub class: Option<String>,
    pub is_tandem: Option<bool>,
    pub brand_id: Option<String>,
    pub brand_name: Option<String>,
    pub brand_text: Option<String>,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub model_text: Option<String>,
    /// How many `flights` rows point at this glider. Drives the "delete
    /// blocked" affordance in the UI.
    pub flights_count: i64,
}

async fn list(
    State(state): State<AppState>,
    identity: Identity,
) -> Result<Json<Vec<MyGliderDto>>, AppError> {
    let rows: Vec<MyGliderDto> = sqlx::query_as::<_, MyGliderDto>(
        "SELECT g.id, \
                g.kind::text  AS kind, \
                g.class::text AS class, \
                g.is_tandem, \
                g.brand_id, \
                b.name        AS brand_name, \
                g.brand_text, \
                g.model_id, \
                m.name        AS model_name, \
                g.model_text, \
                (SELECT COUNT(*) FROM flights f WHERE f.glider_id = g.id) AS flights_count \
           FROM gliders g \
           LEFT JOIN brands b \
             ON b.id = g.brand_id \
           LEFT JOIN glider_models m \
             ON m.brand_id = g.brand_id \
            AND m.kind     = g.kind \
            AND m.id       = g.model_id \
          WHERE g.user_id = $1 \
          ORDER BY g.kind, \
                   COALESCE(b.name, g.brand_text, '') ASC, \
                   COALESCE(m.name, g.model_text, '') ASC, \
                   g.id",
    )
    .bind(identity.user_id)
    .fetch_all(state.pool())
    .await
    .map_err(into_internal)?;

    Ok(Json(rows))
}

async fn delete(
    State(state): State<AppState>,
    identity: Identity,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    // One round-trip to learn (a) does the row exist, (b) does the caller own
    // it, and (c) is anything still pointing at it. We can't tell those apart
    // from a single DELETE's `rows_affected`.
    let row: Option<(i32, i64)> = sqlx::query_as(
        "SELECT g.user_id, \
                (SELECT COUNT(*) FROM flights f WHERE f.glider_id = g.id) \
           FROM gliders g \
          WHERE g.id = $1",
    )
    .bind(id)
    .fetch_optional(state.pool())
    .await
    .map_err(into_internal)?;

    // Same 404 for "no such glider" and "belongs to someone else" so the API
    // doesn't double as an ownership oracle.
    let Some((owner_id, flights_count)) = row else {
        return Err(AppError::NotFound);
    };
    if owner_id != identity.user_id {
        return Err(AppError::NotFound);
    }
    if flights_count > 0 {
        return Err(AppError::Conflict(format!(
            "glider is used by {flights_count} flight{} \
             — reassign or delete those flights first",
            if flights_count == 1 { "" } else { "s" },
        )));
    }

    sqlx::query("DELETE FROM gliders WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(identity.user_id)
        .execute(state.pool())
        .await
        .map_err(into_internal)?;

    Ok(StatusCode::NO_CONTENT)
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
