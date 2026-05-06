//! `GET /tracks/full/:flight_id` — serves the full-resolution compact track
//! blob.
//!
//! The bytes in `flight_tracks.bytes` are the HTTP wire form of a
//! [`TengriFile`](tengri_server::flight::TengriFile): `gzip(bincode(...))`.
//! We stream them straight to the client with `Content-Encoding: gzip` so the
//! browser auto-decompresses; the resulting bincode is self-versioned via the
//! struct's `version` field.
//!
//! Caching: rows are *logically* immutable per `(flight_id, kind)` for a
//! given build version, but a re-encoding cron job may rewrite them. We
//! therefore expose:
//! - a strong `ETag` derived from the bytes (xxh3-64, stored in `etag`),
//! - `Cache-Control: public, max-age=31536000, immutable` for happy-path
//!   browsers,
//! - `If-None-Match` → `304 Not Modified` short-circuit that skips the bytea
//!   fetch entirely (TOAST'd blob never leaves the table).

use axum::{
    Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
    routing::get,
};

use crate::{AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/tracks/full/{id}", get(get_full_track))
}

async fn get_full_track(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    // Two-query fast path: cheap PK lookup for the ETag first, then conditional
    // bytea fetch. The blob lives out-of-line (TOAST), so skipping it on a 304
    // is the whole point of having an ETag column.
    let etag: Option<String> =
        sqlx::query_scalar("SELECT etag FROM flight_tracks WHERE flight_id = $1 AND kind = 'full'")
            .bind(&id)
            .fetch_optional(state.pool())
            .await
            .map_err(anyhow::Error::from)?;

    let Some(etag) = etag else {
        return Err(AppError::NotFound);
    };

    let etag_header = format!("\"{etag}\"");

    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH)
        && if_none_match.as_bytes() == etag_header.as_bytes()
    {
        return Ok(not_modified(&etag_header));
    }

    let bytes: Vec<u8> = sqlx::query_scalar(
        "SELECT bytes FROM flight_tracks WHERE flight_id = $1 AND kind = 'full'",
    )
    .bind(&id)
    .fetch_one(state.pool())
    .await
    .map_err(anyhow::Error::from)?;

    Ok(ok_response(&etag_header, bytes))
}

fn ok_response(etag_header: &str, bytes: Vec<u8>) -> Response {
    let mut resp = Response::new(Body::from(bytes));
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    h.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    h.insert(header::ETAG, etag_header_value(etag_header));
    resp
}

fn not_modified(etag_header: &str) -> Response {
    let mut resp = Response::new(Body::empty());
    *resp.status_mut() = StatusCode::NOT_MODIFIED;
    let h = resp.headers_mut();
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    h.insert(header::ETAG, etag_header_value(etag_header));
    resp
}

fn etag_header_value(etag_header: &str) -> HeaderValue {
    HeaderValue::from_str(etag_header).expect("etag header value must be ASCII by construction")
}
