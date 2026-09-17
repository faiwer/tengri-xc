//! `GET /tracks/{id}/og.jpg` — the flight's track, plus its scored route's
//! waypoints, rendered as a JPEG for link previews.
//!
//! The first request draws the picture and stores it in `flight_images`; later
//! ones serve those bytes. Re-scoring a flight deletes the row, so the next
//! request redraws. Concurrent requests for the same cold flight go through
//! [`RenderGate`], which lets one of them draw while the rest wait for the
//! result — the satellite fetch alone is worth not doing four times over.
//!
//! [`RenderGate`]: crate::flight::image::RenderGate
//!
//! `Cache-Control: no-cache` because a re-score changes the bytes under a URL
//! that stays the same; the `ETag` keeps that revalidation to a PK lookup and
//! an empty 304 rather than a re-download.

use axum::{
    Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
    routing::get,
};

use crate::{
    AppError, AppState,
    flight::{
        etag_for,
        image::{Turn, fetch_basemap, render_flight_image},
        store::{
            StoredImage, fetch_full_track, fetch_image, fetch_image_etag, fetch_main_route,
            upsert_image,
        },
    },
};

pub fn router() -> Router<AppState> {
    Router::new().route("/tracks/{id}/og.jpg", get(get_track_image))
}

async fn get_track_image(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    // Etag first, so a revalidation never detoasts the blob.
    if let Some(etag) = fetch_image_etag(state.pool(), &id).await? {
        let etag_header = quoted(&etag);
        if headers
            .get(header::IF_NONE_MATCH)
            .is_some_and(|sent| sent.as_bytes() == etag_header.as_bytes())
        {
            return Ok(not_modified(&etag_header));
        }
        if let Some(image) = fetch_image(state.pool(), &id).await? {
            return Ok(ok_response(image));
        }
    }

    Ok(ok_response(draw(&state, &id).await?))
}

/// One caller draws, the rest wait for it and read what it stored.
async fn draw(state: &AppState, id: &str) -> Result<StoredImage, AppError> {
    match state.image_renders().enter(id) {
        // `_lease` has to outlive the render: dropping it is what releases the
        // followers, and they should find a stored row when it does.
        Turn::Render(_lease) => render_and_store(state, id).await,
        Turn::Wait(mut leader) => {
            // The lease signals by dropping its sender, so an error here is
            // the expected ending, not a problem.
            let _ = leader.recv().await;
            match fetch_image(state.pool(), id).await? {
                Some(image) => Ok(image),
                // Nothing stored means the leader failed. Draw it ourselves
                // rather than inherit an error we never saw.
                None => render_and_store(state, id).await,
            }
        }
    }
}

async fn render_and_store(state: &AppState, id: &str) -> Result<StoredImage, AppError> {
    let track = fetch_full_track(state.pool(), id)
        .await?
        .ok_or(AppError::NotFound)?;
    let route = fetch_main_route(state.pool(), id).await?;
    let basemap = match state.satellite_map() {
        Some(satellite) => fetch_basemap(satellite, &track).await,
        None => None,
    };

    let bytes = tokio::task::spawn_blocking(move || {
        render_flight_image(&track, route.as_ref(), basemap.as_ref())
    })
    .await
    .map_err(anyhow::Error::from)??;

    let etag = etag_for(&bytes);
    upsert_image(state.pool(), id, &bytes, &etag).await?;
    Ok(StoredImage { bytes, etag })
}

fn ok_response(image: StoredImage) -> Response {
    let etag_header = quoted(&image.etag);
    let mut response = Response::new(Body::from(image.bytes));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::ETAG, etag_header_value(&etag_header));
    response
}

fn not_modified(etag_header: &str) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NOT_MODIFIED;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::ETAG, etag_header_value(etag_header));
    response
}

fn quoted(etag: &str) -> String {
    format!("\"{etag}\"")
}

fn etag_header_value(etag_header: &str) -> HeaderValue {
    HeaderValue::from_str(etag_header).expect("etag header value must be ASCII by construction")
}
