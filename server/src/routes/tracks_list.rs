//! `GET /tracks` — cursor-paginated list of flights, newest first by
//! `takeoff_at`.
//!
//! Page shape: `{ items: [...], next_cursor: string|null }`. Clients pass
//! `next_cursor` back as `?cursor=...` until they receive `null`. The cursor
//! is opaque — internally a 12-byte packed `(u32 takeoff_at, [u8; 8]
//! flight_id)` rendered as 16 base64url chars — and clients must treat it
//! as such. Encoding may change without an API revision.
//!
//! Sort order is `(takeoff_at DESC, flight_id DESC)`. The trailing `id`
//! column is purely a tie-break for flights that share `takeoff_at` to the
//! second; without it, paginating across a tie would skip rows or yield
//! duplicates. The `flights_takeoff_idx` index covers the leading column,
//! and the PK on `flights.id` makes the row-comparison index-friendly.
//!
//! No filters, no alternate sorts — those belong to a follow-up. The
//! cursor format is intentionally extensible: a future change can add a
//! version byte without breaking this v0.

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/tracks", get(list_tracks))
}

/// NanoID length used for `flights.id`. The cursor packs the id as fixed-
/// width bytes; if this ever changes, bump the cursor format.
const FLIGHT_ID_LEN: usize = 8;

/// Default page size when the client doesn't provide `?limit=`.
const DEFAULT_LIMIT: u32 = 25;

/// Hard cap on `?limit=`. Above this we return `400`. The cap exists to
/// keep response sizes and DB scans bounded; loosening it requires
/// thinking about response payloads, not just bumping the number.
const MAX_LIMIT: u32 = 100;

#[derive(Deserialize)]
struct ListQuery {
    limit: Option<u32>,
    cursor: Option<String>,
}

#[derive(Serialize)]
struct ListResponse {
    items: Vec<Item>,
    /// Opaque cursor for the next page, or `null` on the last page. Pass
    /// it back verbatim as `?cursor=...`. Set only when the page filled to
    /// `limit`; on a short final page we know there's no more.
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct Item {
    pilot: Pilot,
    track: TrackRef,
}

#[derive(Serialize)]
struct Pilot {
    id: i32,
    name: String,
}

#[derive(Serialize)]
struct TrackRef {
    id: String,
    /// Unix epoch seconds (UTC). Same wire shape as `/tracks/{id}/md`.
    takeoff_at: i64,
    /// Whole seconds, from the `flights.duration_s` generated column.
    duration: i32,
}

async fn list_tracks(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, AppError> {
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(AppError::BadRequest(format!(
            "limit must be between 1 and {MAX_LIMIT}",
        )));
    }

    // Fetch `limit + 1` so a full page tells us *for sure* whether there
    // is at least one more row past the end. The extra row is dropped
    // before serialising; without it, the client would have to make one
    // more empty round-trip every time the total count is an exact
    // multiple of `limit` to discover the end of the list.
    let probe = limit as i64 + 1;
    let mut rows: Vec<(String, i64, i32, i32, String)> = match q.cursor.as_deref() {
        None => sqlx::query_as(
            "SELECT f.id, \
                        EXTRACT(EPOCH FROM f.takeoff_at)::bigint, \
                        f.duration_s, \
                        u.id, \
                        u.name \
                 FROM flights f \
                 JOIN users u ON u.id = f.user_id \
                 ORDER BY f.takeoff_at DESC, f.id DESC \
                 LIMIT $1",
        )
        .bind(probe)
        .fetch_all(state.pool())
        .await
        .map_err(anyhow::Error::from)?,
        Some(raw) => {
            let (cursor_t, cursor_id) = decode_cursor(raw)?;
            // Row-comparison `(takeoff_at, id) < (...)` is the standard
            // keyset-pagination predicate: it picks up exactly where the
            // last page left off, without `OFFSET`'s O(n) skip cost, and
            // remains correct across `takeoff_at` ties because `id`
            // breaks them.
            sqlx::query_as(
                "SELECT f.id, \
                        EXTRACT(EPOCH FROM f.takeoff_at)::bigint, \
                        f.duration_s, \
                        u.id, \
                        u.name \
                 FROM flights f \
                 JOIN users u ON u.id = f.user_id \
                 WHERE (f.takeoff_at, f.id) < (to_timestamp($1), $2) \
                 ORDER BY f.takeoff_at DESC, f.id DESC \
                 LIMIT $3",
            )
            .bind(cursor_t as i64)
            .bind(cursor_id)
            .bind(probe)
            .fetch_all(state.pool())
            .await
            .map_err(anyhow::Error::from)?
        }
    };

    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.truncate(limit as usize);
    }
    let next_cursor = if has_more {
        let (last_id, last_takeoff, _, _, _) = rows.last().expect("has_more implies non-empty");
        Some(encode_cursor(*last_takeoff as u32, last_id))
    } else {
        None
    };

    let items = rows
        .into_iter()
        .map(
            |(flight_id, takeoff_at, duration, user_id, user_name)| Item {
                pilot: Pilot {
                    id: user_id,
                    name: user_name,
                },
                track: TrackRef {
                    id: flight_id,
                    takeoff_at,
                    duration,
                },
            },
        )
        .collect();

    Ok(Json(ListResponse { items, next_cursor }))
}

/// Pack `(takeoff_at, flight_id)` into 12 bytes and base64url-encode.
///
/// Layout: `[0..4]` big-endian `u32` epoch seconds, `[4..12]` raw ASCII
/// bytes of the flight id. Big-endian because it's the conventional
/// network byte order for packed integers and makes the cursor sort
/// lexicographically by time when one ever cares to look (debugging).
fn encode_cursor(takeoff_at: u32, flight_id: &str) -> String {
    debug_assert_eq!(
        flight_id.len(),
        FLIGHT_ID_LEN,
        "flight_id must be {FLIGHT_ID_LEN} bytes",
    );
    let mut buf = [0u8; 4 + FLIGHT_ID_LEN];
    buf[0..4].copy_from_slice(&takeoff_at.to_be_bytes());
    buf[4..].copy_from_slice(flight_id.as_bytes());
    URL_SAFE_NO_PAD.encode(buf)
}

/// Decode a base64url cursor into `(takeoff_at, flight_id)`. Rejects:
/// - bad base64;
/// - decoded length ≠ 12 bytes;
/// - non-NanoID-alphabet bytes in the id slice (so junk can't be
///   smuggled into a SQL parameter).
fn decode_cursor(s: &str) -> Result<(u32, String), AppError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| AppError::BadRequest("malformed cursor".into()))?;
    if bytes.len() != 4 + FLIGHT_ID_LEN {
        return Err(AppError::BadRequest("malformed cursor".into()));
    }

    let takeoff_at = u32::from_be_bytes(
        bytes[0..4]
            .try_into()
            .expect("4 bytes by length check above"),
    );

    let id_bytes = &bytes[4..];
    if !id_bytes.iter().all(|b| is_nanoid_byte(*b)) {
        return Err(AppError::BadRequest("malformed cursor".into()));
    }
    // ASCII subset by the predicate above, so UTF-8 conversion is
    // infallible — but go through `from_utf8` rather than `unsafe` to
    // keep the decoder panic-free under any input.
    let id = std::str::from_utf8(id_bytes)
        .map_err(|_| AppError::BadRequest("malformed cursor".into()))?
        .to_owned();

    Ok((takeoff_at, id))
}

fn is_nanoid_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips() {
        let encoded = encode_cursor(1_777_887_122, "ABCD1234");
        assert_eq!(encoded.len(), 16, "12 bytes -> 16 base64url chars");
        let (t, id) = decode_cursor(&encoded).unwrap();
        assert_eq!(t, 1_777_887_122);
        assert_eq!(id, "ABCD1234");
    }

    #[test]
    fn cursor_rejects_bad_base64() {
        let err = decode_cursor("not base64!!!").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn cursor_rejects_wrong_length() {
        // 9 bytes -> 12 base64url chars, well-formed base64 but wrong
        // payload size for our format.
        let short = URL_SAFE_NO_PAD.encode([0u8; 9]);
        assert!(matches!(
            decode_cursor(&short),
            Err(AppError::BadRequest(_)),
        ));
    }

    #[test]
    fn cursor_rejects_non_alphabet_id() {
        // Valid u32 prefix, then a NUL byte in the id slice.
        let mut buf = [0u8; 12];
        buf[0..4].copy_from_slice(&1u32.to_be_bytes());
        buf[4] = 0; // not in the NanoID alphabet
        let bad = URL_SAFE_NO_PAD.encode(buf);
        assert!(matches!(decode_cursor(&bad), Err(AppError::BadRequest(_))));
    }
}
