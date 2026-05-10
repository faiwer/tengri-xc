//! `GET /tracks` — cursor-paginated list of flights, newest first by
//! `takeoff_at`.
//!
//! Page shape: `{ items: [...], next_cursor: string|null }`. Clients pass
//! `next_cursor` back as `?cursor=...` until they receive `null`. The cursor
//! is opaque — internally a length-prefixed pack of `(u32 takeoff_at,
//! u8 id_len, ascii flight_id)` rendered as base64url — and clients must
//! treat it as such. Encoding may change without an API revision.
//!
//! Sort order is `(takeoff_at DESC, flight_id DESC)`. The trailing `id`
//! column is purely a tie-break for flights that share `takeoff_at` to the
//! second; without it, paginating across a tie would skip rows or yield
//! duplicates. The `flights_takeoff_idx` index covers the leading column,
//! and the PK on `flights.id` makes the row-comparison index-friendly.
//!
//! No filters, no alternate sorts — those belong to a follow-up.

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use crate::{
    AppError, AppState,
    db::{Order, Sql},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/tracks", get(list_tracks))
}

/// Hard cap on the flight-id length we'll pack into a cursor. The id
/// goes in as a `u8`-length-prefixed slice, so the format-level cap
/// is 255; we keep the limit a touch lower to leave headroom and to
/// reject obviously-bogus inputs early. Native NanoIDs are 8 bytes;
/// the leonardo importer produces `LEO-<n>` which currently tops out
/// at 8 (e.g. `LEO-1350`) and won't realistically exceed `LEO-` plus
/// 19 digits even at u64 max.
const FLIGHT_ID_MAX: usize = 32;

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
    /// ISO 3166-1 alpha-2 country code from the user's profile, or
    /// `None` if no profile / no country recorded. The client renders
    /// it as a flag emoji with a hover title.
    country: Option<String>,
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
    let cursor = q.cursor.as_deref().map(decode_cursor).transpose()?;

    let mut query = Sql::select(&[
        "f.id",
        "EXTRACT(EPOCH FROM f.takeoff_at)::bigint",
        "f.duration_s",
        "u.id",
        "u.name",
        "p.country",
    ])
    .from("flights f")
    .join("users u", "u.id = f.user_id")
    // `LEFT JOIN user_profiles` because country is profile-side and
    // optional; users without a profile row (or without a country)
    // still appear with `pilot.country = null`.
    .left_join("user_profiles p", "p.user_id = u.id")
    .order_by("f.takeoff_at", Order::Desc)
    .order_by("f.id", Order::Desc)
    .limit(probe);

    // Row-comparison `(takeoff_at, id) < (...)` is the standard
    // keyset-pagination predicate: picks up where the last page left
    // off without `OFFSET`'s O(n) skip cost, and stays correct across
    // `takeoff_at` ties because `id` breaks them.
    if let Some((cursor_t, cursor_id)) = cursor {
        query.and_where(
            "(f.takeoff_at, f.id) < (to_timestamp($), $)",
            (cursor_t as i64, cursor_id),
        );
    }

    let mut rows: Vec<(String, i64, i32, i32, String, Option<String>)> = query
        .fetch_all(state.pool())
        .await
        .map_err(anyhow::Error::from)?;

    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.truncate(limit as usize);
    }
    let next_cursor = if has_more {
        let (last_id, last_takeoff, _, _, _, _) = rows.last().expect("has_more implies non-empty");
        Some(encode_cursor(*last_takeoff as u32, last_id))
    } else {
        None
    };

    let items = rows
        .into_iter()
        .map(
            |(flight_id, takeoff_at, duration, user_id, user_name, country)| Item {
                pilot: Pilot {
                    id: user_id,
                    name: user_name,
                    country,
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

/// Pack `(takeoff_at, flight_id)` and base64url-encode. The id is
/// length-prefixed so the cursor self-describes — flight ids today
/// come in two flavours (8-byte NanoIDs from `tengri add`, variable
/// `LEO-<n>` from the leonardo importer) and a fixed-width layout
/// would have to know which is which.
///
/// Layout: `[0..4]` big-endian `u32` epoch seconds, `[4]` `u8` id
/// length, `[5..5+len]` raw ASCII bytes of the flight id.
/// Big-endian for the timestamp because it's conventional network
/// byte order and makes the cursor sort lexicographically by time
/// when one ever cares to look (debugging).
fn encode_cursor(takeoff_at: u32, flight_id: &str) -> String {
    debug_assert!(
        flight_id.len() <= FLIGHT_ID_MAX,
        "flight_id is {} bytes, > FLIGHT_ID_MAX={FLIGHT_ID_MAX}",
        flight_id.len(),
    );
    let id = flight_id.as_bytes();
    let mut buf = Vec::with_capacity(5 + id.len());
    buf.extend_from_slice(&takeoff_at.to_be_bytes());
    buf.push(id.len() as u8);
    buf.extend_from_slice(id);
    URL_SAFE_NO_PAD.encode(buf)
}

/// Decode a base64url cursor into `(takeoff_at, flight_id)`. Rejects:
/// - bad base64;
/// - decoded length not consistent with the embedded id-length byte;
/// - id length > [`FLIGHT_ID_MAX`];
/// - non-id-alphabet bytes in the id slice (so junk can't be smuggled
///   into a SQL parameter).
fn decode_cursor(s: &str) -> Result<(u32, String), AppError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| AppError::BadRequest("malformed cursor".into()))?;
    if bytes.len() < 5 {
        return Err(AppError::BadRequest("malformed cursor".into()));
    }

    let takeoff_at = u32::from_be_bytes(
        bytes[0..4]
            .try_into()
            .expect("4 bytes by length check above"),
    );

    let id_len = bytes[4] as usize;
    if id_len == 0 || id_len > FLIGHT_ID_MAX || bytes.len() != 5 + id_len {
        return Err(AppError::BadRequest("malformed cursor".into()));
    }

    let id_bytes = &bytes[5..5 + id_len];
    if !id_bytes.iter().all(|b| is_id_byte(*b)) {
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

/// Bytes we accept inside an opaque cursor's id slice. The union of
/// the NanoID alphabet (`[A-Za-z0-9_-]`) and the leonardo importer's
/// `LEO-<digits>` shape (already a subset of NanoID's). A separate
/// predicate from the NanoID generator's alphabet because we don't
/// want to imply the cursor enforces a generator format.
fn is_id_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips_nanoid() {
        let (t, id) = decode_cursor(&encode_cursor(1_777_887_122, "ABCD1234")).unwrap();
        assert_eq!(t, 1_777_887_122);
        assert_eq!(id, "ABCD1234");
    }

    #[test]
    fn cursor_round_trips_leonardo_id() {
        let (t, id) = decode_cursor(&encode_cursor(1_777_887_122, "LEO-1350")).unwrap();
        assert_eq!(t, 1_777_887_122);
        assert_eq!(id, "LEO-1350");
    }

    #[test]
    fn cursor_round_trips_short_id() {
        let (t, id) = decode_cursor(&encode_cursor(1, "LEO-7")).unwrap();
        assert_eq!(t, 1);
        assert_eq!(id, "LEO-7");
    }

    #[test]
    fn cursor_rejects_bad_base64() {
        let err = decode_cursor("not base64!!!").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn cursor_rejects_too_short_to_hold_header() {
        let short = URL_SAFE_NO_PAD.encode([0u8; 4]);
        assert!(matches!(
            decode_cursor(&short),
            Err(AppError::BadRequest(_)),
        ));
    }

    #[test]
    fn cursor_rejects_length_mismatch() {
        // Header says id is 8 bytes long, payload only carries 4.
        let mut buf = vec![0u8; 4];
        buf.push(8);
        buf.extend_from_slice(b"ABCD");
        assert!(matches!(
            decode_cursor(&URL_SAFE_NO_PAD.encode(buf)),
            Err(AppError::BadRequest(_)),
        ));
    }

    #[test]
    fn cursor_rejects_oversized_id() {
        let mut buf = vec![0u8; 4];
        buf.push((FLIGHT_ID_MAX + 1) as u8);
        buf.extend(std::iter::repeat_n(b'A', FLIGHT_ID_MAX + 1));
        assert!(matches!(
            decode_cursor(&URL_SAFE_NO_PAD.encode(buf)),
            Err(AppError::BadRequest(_)),
        ));
    }

    #[test]
    fn cursor_rejects_non_alphabet_id() {
        let mut buf = vec![0u8; 4];
        buf.push(4);
        buf.extend_from_slice(&[b'A', 0, b'B', b'C']);
        assert!(matches!(
            decode_cursor(&URL_SAFE_NO_PAD.encode(buf)),
            Err(AppError::BadRequest(_)),
        ));
    }
}
