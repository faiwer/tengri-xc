//! `GET /tracks/{id}/og.jpg` — the flight's track, plus its scored route's
//! waypoints, rendered as a JPEG for link previews.
//!
//! Rendered per request for now, hence `no-cache`; persisting the bytes is a
//! later step. That also means the satellite tiles are refetched per request,
//! on a budget short enough that a slow imagery service costs the caller a
//! plain white backdrop rather than a timeout.

use axum::{
    Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderValue, header},
    response::Response,
    routing::get,
};

use crate::{
    AppError, AppState,
    flight::{
        image::{fetch_basemap, render_flight_image},
        store::{fetch_full_track, fetch_main_route},
    },
};

pub fn router() -> Router<AppState> {
    Router::new().route("/tracks/{id}/og.jpg", get(get_track_image))
}

async fn get_track_image(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let track = fetch_full_track(state.pool(), &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let route = fetch_main_route(state.pool(), &id).await?;
    let basemap = match state.satellite_map() {
        Some(satellite) => fetch_basemap(satellite, &track).await,
        None => None,
    };

    let jpeg = tokio::task::spawn_blocking(move || {
        render_flight_image(&track, route.as_ref(), basemap.as_ref())
    })
    .await
    .map_err(anyhow::Error::from)??;

    let mut response = Response::new(Body::from(jpeg));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    Ok(response)
}
