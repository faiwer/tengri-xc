//! `GET /tracks/{id}/md` — sidecar metadata for a track.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use serde::Serialize;

use crate::{AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/tracks/{id}/md", get(get_track_md))
}

#[derive(Serialize)]
struct TrackMd {
    id: String,
    pilot: Pilot,
    /// Unix epoch seconds (UTC). The DB stores `timestamptz`; we project it as
    /// `bigint` epoch so the wire format stays numeric and the client can do
    /// `new Date(seconds * 1000)` without parsing strings.
    takeoff_at: i64,
    landed_at: i64,
}

#[derive(Serialize)]
struct Pilot {
    name: String,
}

async fn get_track_md(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TrackMd>, AppError> {
    let row: Option<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT f.id, u.name, \
                EXTRACT(EPOCH FROM f.takeoff_at)::bigint, \
                EXTRACT(EPOCH FROM f.landed_at)::bigint \
         FROM flights f \
         JOIN users u ON u.id = f.user_id \
         WHERE f.id = $1",
    )
    .bind(&id)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    let Some((flight_id, pilot_name, takeoff_at, landed_at)) = row else {
        return Err(AppError::NotFound);
    };

    Ok(Json(TrackMd {
        id: flight_id,
        pilot: Pilot { name: pilot_name },
        takeoff_at,
        landed_at,
    }))
}
