//! `GET /tracks/{id}/md` — sidecar metadata for a track.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use serde::Serialize;

use crate::{
    AppError, AppState,
    flight::{Route, store::fetch_scored_routes},
};
use tengri_geo::PointDegrees;

pub fn router() -> Router<AppState> {
    Router::new().route("/tracks/{id}/md", get(get_track_md))
}

#[derive(Serialize)]
struct TrackMd {
    id: String,
    pilot: Pilot,
    glider: Glider,
    /// Unix epoch seconds (UTC). The DB stores `timestamptz`; we project it as
    /// `bigint` epoch so the wire format stays numeric and the client can do
    /// `new Date(seconds * 1000)` without parsing strings.
    takeoff_at: i64,
    landing_at: i64,
    /// IANA timezone names at the takeoff/landing fixes.
    takeoff_timezone: String,
    landing_timezone: String,
    takeoff: PointDegrees,
    landing: PointDegrees,
    /// Wire-track size as a fraction of the gzipped source (0.0..1.0).
    compression_ratio: f32,
    routes: Vec<Route>,
    /// Best-scoring or manually chosen route summary; `null` when the flight
    /// has not been scored yet.
    main_route: Option<MainRoute>,
}

#[derive(Serialize)]
struct MainRoute {
    /// Surrogate PK of the `routes` row.
    id: i64,
    route_type: String,
    /// XC points.
    score: f64,
    /// Distance in metres.
    distance: i32,
}

#[derive(Serialize)]
struct Pilot {
    name: String,
    /// ISO 3166-1 alpha-2 country code from the user's profile, or
    /// `None` if no profile / no country recorded. The client renders
    /// it as a flag emoji prepended to the pilot name.
    country: Option<String>,
}

#[derive(Serialize)]
struct Glider {
    brand_id: String,
    brand_name: String,
    model_id: String,
    model_name: String,
}

async fn get_track_md(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TrackMd>, AppError> {
    // `LEFT JOIN user_profiles` because country is profile-side and
    // optional; users without a profile row still resolve with a
    // `null` country rather than dropping the flight to a 404.
    let row: Option<TrackMdRow> = sqlx::query_as(
        "SELECT f.id, \
                u.name AS pilot_name, p.country AS pilot_country, \
                f.brand_id, b.name AS brand_name, f.model_id, m.name AS model_name, \
                EXTRACT(EPOCH FROM f.takeoff_at)::bigint AS takeoff_at, \
                EXTRACT(EPOCH FROM f.landing_at)::bigint AS landing_at, \
                f.takeoff_timezone, f.landing_timezone, \
                ST_Y(f.takeoff_point::geometry) AS takeoff_lat, \
                ST_X(f.takeoff_point::geometry) AS takeoff_lon, \
                ST_Y(f.landing_point::geometry) AS landing_lat, \
                ST_X(f.landing_point::geometry) AS landing_lon, \
                t.compression_ratio, \
                f.main_route_id, \
                f.main_route_type::text AS main_route_type, \
                f.main_score::float8 AS main_score, \
                f.main_distance \
         FROM flights f \
         JOIN users u ON u.id = f.user_id \
         LEFT JOIN user_profiles p ON p.user_id = u.id \
         JOIN brands b ON b.id = f.brand_id \
         JOIN models m ON m.brand_id = f.brand_id \
                      AND m.kind = f.kind \
                      AND m.id = f.model_id \
         JOIN flight_tracks t ON t.flight_id = f.id AND t.kind = 'full' \
         WHERE f.id = $1",
    )
    .bind(&id)
    .fetch_optional(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    let Some(row) = row else {
        return Err(AppError::NotFound);
    };

    let routes = fetch_scored_routes(state.pool(), &row.id).await?;

    Ok(Json(TrackMd {
        id: row.id,
        pilot: Pilot {
            name: row.pilot_name,
            country: row.pilot_country,
        },
        glider: Glider {
            brand_id: row.brand_id,
            brand_name: row.brand_name,
            model_id: row.model_id,
            model_name: row.model_name,
        },
        takeoff_at: row.takeoff_at,
        landing_at: row.landing_at,
        takeoff_timezone: row.takeoff_timezone,
        landing_timezone: row.landing_timezone,
        takeoff: PointDegrees {
            lat: row.takeoff_lat,
            lon: row.takeoff_lon,
        },
        landing: PointDegrees {
            lat: row.landing_lat,
            lon: row.landing_lon,
        },
        compression_ratio: row.compression_ratio,
        routes,
        main_route: match (
            row.main_route_id,
            row.main_route_type,
            row.main_score,
            row.main_distance,
        ) {
            (Some(id), Some(route_type), Some(score), Some(distance)) => Some(MainRoute {
                id,
                route_type,
                score,
                distance,
            }),
            _ => None,
        },
    }))
}

#[derive(sqlx::FromRow)]
struct TrackMdRow {
    id: String,
    pilot_name: String,
    pilot_country: Option<String>,
    brand_id: String,
    brand_name: String,
    model_id: String,
    model_name: String,
    takeoff_at: i64,
    landing_at: i64,
    takeoff_timezone: String,
    landing_timezone: String,
    takeoff_lat: f64,
    takeoff_lon: f64,
    landing_lat: f64,
    landing_lon: f64,
    compression_ratio: f32,
    main_route_id: Option<i64>,
    main_route_type: Option<String>,
    main_score: Option<f64>,
    main_distance: Option<i32>,
}
