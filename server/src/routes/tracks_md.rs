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
    landing_at: i64,
    /// UTC offsets in whole seconds at the takeoff/landing fixes, computed at
    /// ingest from the fix coordinates and the historical `tzdb` rules valid on
    /// the flight's date.
    takeoff_offset: i32,
    landing_offset: i32,
    takeoff: Point,
    landing: Point,
    /// Wire-track size as a fraction of the gzipped source (0.0..1.0).
    compression_ratio: f32,
}

#[derive(Serialize)]
struct Pilot {
    name: String,
    /// ISO 3166-1 alpha-2 country code from the user's profile, or
    /// `None` if no profile / no country recorded. The client renders
    /// it as a flag emoji prepended to the pilot name.
    country: Option<String>,
}

/// Decimal degrees on WGS-84. The DB carries the column as `geography(Point,
/// 4326)`; we cast to `geometry` only to use `ST_X` / `ST_Y`, which
/// spatial-types-wise is a no-op for points.
#[derive(Serialize)]
struct Point {
    lat: f64,
    lon: f64,
}

async fn get_track_md(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TrackMd>, AppError> {
    // `LEFT JOIN user_profiles` because country is profile-side and
    // optional; users without a profile row still resolve with a
    // `null` country rather than dropping the flight to a 404.
    let row: Option<TrackMdRow> = sqlx::query_as(
        "SELECT f.id, u.name, p.country, \
                EXTRACT(EPOCH FROM f.takeoff_at)::bigint, \
                EXTRACT(EPOCH FROM f.landing_at)::bigint, \
                f.takeoff_offset, \
                f.landing_offset, \
                ST_Y(f.takeoff_point::geometry), \
                ST_X(f.takeoff_point::geometry), \
                ST_Y(f.landing_point::geometry), \
                ST_X(f.landing_point::geometry), \
                t.compression_ratio \
         FROM flights f \
         JOIN users u ON u.id = f.user_id \
         LEFT JOIN user_profiles p ON p.user_id = u.id \
         JOIN flight_tracks t ON t.flight_id = f.id AND t.kind = 'full' \
         WHERE f.id = $1",
    )
    .bind(&id)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    let Some((
        flight_id,
        pilot_name,
        pilot_country,
        takeoff_at,
        landing_at,
        takeoff_offset,
        landing_offset,
        takeoff_lat,
        takeoff_lon,
        landing_lat,
        landing_lon,
        compression_ratio,
    )) = row
    else {
        return Err(AppError::NotFound);
    };

    Ok(Json(TrackMd {
        id: flight_id,
        pilot: Pilot {
            name: pilot_name,
            country: pilot_country,
        },
        takeoff_at,
        landing_at,
        takeoff_offset,
        landing_offset,
        takeoff: Point {
            lat: takeoff_lat,
            lon: takeoff_lon,
        },
        landing: Point {
            lat: landing_lat,
            lon: landing_lon,
        },
        compression_ratio,
    }))
}

/// Concrete row tuple: `(flight_id, pilot_name, pilot_country, takeoff_at,
/// landing_at, takeoff_offset, landing_offset, takeoff_lat, takeoff_lon,
/// landing_lat, landing_lon, compression_ratio)`. Aliased because the literal
/// tuple is too long to inline at the call site without harming readability.
type TrackMdRow = (
    String,
    String,
    Option<String>,
    i64,
    i64,
    i32,
    i32,
    f64,
    f64,
    f64,
    f64,
    f32,
);
