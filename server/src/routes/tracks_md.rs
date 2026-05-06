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
}

#[derive(Serialize)]
struct Pilot {
    name: String,
}

async fn get_track_md(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TrackMd>, AppError> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT f.id, u.name \
         FROM flights f \
         JOIN users u ON u.id = f.user_id \
         WHERE f.id = $1",
    )
    .bind(&id)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    let Some((flight_id, pilot_name)) = row else {
        return Err(AppError::NotFound);
    };

    Ok(Json(TrackMd {
        id: flight_id,
        pilot: Pilot { name: pilot_name },
    }))
}
