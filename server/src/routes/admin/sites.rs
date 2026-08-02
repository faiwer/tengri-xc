//! `/admin/sites` — list the takeoff-sites directory. Requires the
//! `MANAGE_SITES` bit.
//!
//! - `GET /admin/sites?q=&cursor=&limit=` — keyset-paginated list. Sort:
//!   alphabetical, `(name ASC, id ASC)`. `q` matches `name` case-insensitively
//!   (`ILIKE`); empty / missing means no filter. The cursor is opaque —
//!   internally `[name_len][name][id]` rendered as base64url, mirroring the
//!   length-prefixed id cursor in `tracks_list`.
//!
//! Search uses `ILIKE` (no trigram index yet); the sites table is small enough
//! that a Seq Scan is fine.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use crate::{
    AppError, AppState,
    auth::{Identity, require_permission},
    db::{Order, Sql, like_contains},
    user::Permissions,
    validation::FieldErrors,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/sites", get(list).post(create))
        .route("/admin/sites/{id}", patch(update))
}

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 100;
/// Hard cap on `?q=`. Above this we 400. Avoids someone shipping a huge
/// pattern through the SQL parameter and pinning a worker on the `ILIKE` scan.
const MAX_QUERY_LEN: usize = 50;
/// Hard cap on the site name we'll pack into a cursor. Site names are short;
/// the format-level cap is the `u8` length prefix (255).
const SITE_NAME_MAX: usize = 128;

#[derive(Debug, Deserialize)]
struct ListQuery {
    #[serde(default)]
    q: Option<String>,
    cursor: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ListResponse {
    items: Vec<ListItem>,
    /// Opaque cursor for the next page, or `null` on the last page. Pass it
    /// back verbatim as `?cursor=...`.
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ListItem {
    id: i32,
    name: String,
    /// ISO 3166-1 alpha-2 of the site, or `None`. Rendered as a flag in the
    /// Name cell.
    country: Option<String>,
    /// Decimal degrees on WGS-84.
    lat: f64,
    lng: f64,
}

async fn list(
    State(state): State<AppState>,
    identity: Identity,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, AppError> {
    require_permission(&identity, Permissions::MANAGE_SITES)?;

    let limit = q.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(AppError::BadRequest(format!(
            "limit must be between 1 and {MAX_LIMIT}",
        )));
    }
    let probe = limit as i64 + 1;

    let needle = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(s) = needle
        && s.len() > MAX_QUERY_LEN
    {
        return Err(AppError::BadRequest(format!(
            "q must be at most {MAX_QUERY_LEN} characters",
        )));
    }
    let pattern = needle.map(like_contains);

    let cursor = q.cursor.as_deref().map(decode_cursor).transpose()?;

    let mut query = Sql::select(&[
        "si.id",
        "si.name",
        "si.country",
        "ST_Y(si.point::geometry) AS lat",
        "ST_X(si.point::geometry) AS lng",
    ])
    .from("sites si")
    .order_by("si.name", Order::Asc)
    .order_by("si.id", Order::Asc)
    .limit(probe);

    // Row-comparison against the `(name, id)` sort tuple. Both ASC, so `>
    // cursor` picks the rows that come *after* the cursor row.
    if let Some((c_name, c_id)) = cursor {
        query.and_where("(si.name, si.id) > ($, $)", (c_name, c_id));
    }
    if let Some(pat) = pattern.as_deref() {
        query.and_where("si.name ILIKE $ ESCAPE '\\'", (pat,));
    }

    let mut items: Vec<ListItem> = query.fetch_all(state.pool()).await.map_err(into_internal)?;

    let has_more = items.len() > limit as usize;
    if has_more {
        items.truncate(limit as usize);
    }

    let next_cursor = if has_more {
        let last = items.last().expect("has_more implies non-empty");
        Some(encode_cursor(&last.name, last.id))
    } else {
        None
    };

    Ok(Json(ListResponse { items, next_cursor }))
}

/// Create + update share this body. The form always submits every field, so an
/// update is a full replace rather than a partial patch.
#[derive(Debug, Deserialize)]
struct SiteInput {
    name: String,
    /// Decimal degrees on WGS-84.
    lat: f64,
    lng: f64,
    #[serde(default)]
    country: Option<String>,
}

/// Validated + normalised [`SiteInput`]: `name` trimmed, `country` upper-cased
/// with empties collapsed to `None`.
struct ValidSite {
    name: String,
    lat: f64,
    lng: f64,
    country: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    identity: Identity,
    Json(input): Json<SiteInput>,
) -> Result<Json<ListItem>, AppError> {
    require_permission(&identity, Permissions::MANAGE_SITES)?;
    let site = validate_site_input(input).map_err(AppError::Validation)?;

    let id: i32 = sqlx::query_scalar(
        "INSERT INTO sites (name, country, point) \
         VALUES ($1, $2, ST_SetSRID(ST_MakePoint($3, $4), 4326)::geography) \
         RETURNING id",
    )
    .bind(&site.name)
    .bind(&site.country)
    .bind(site.lng)
    .bind(site.lat)
    .fetch_one(state.pool())
    .await
    .map_err(into_internal)?;

    Ok(Json(site.into_list_item(id)))
}

async fn update(
    State(state): State<AppState>,
    identity: Identity,
    Path(id): Path<i32>,
    Json(input): Json<SiteInput>,
) -> Result<Json<ListItem>, AppError> {
    require_permission(&identity, Permissions::MANAGE_SITES)?;
    let site = validate_site_input(input).map_err(AppError::Validation)?;

    let result = sqlx::query(
        "UPDATE sites \
         SET name = $1, country = $2, \
             point = ST_SetSRID(ST_MakePoint($3, $4), 4326)::geography \
         WHERE id = $5",
    )
    .bind(&site.name)
    .bind(&site.country)
    .bind(site.lng)
    .bind(site.lat)
    .bind(id)
    .execute(state.pool())
    .await
    .map_err(into_internal)?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(site.into_list_item(id)))
}

impl ValidSite {
    fn into_list_item(self, id: i32) -> ListItem {
        ListItem {
            id,
            name: self.name,
            country: self.country,
            lat: self.lat,
            lng: self.lng,
        }
    }
}

/// Field keys (`name`/`lat`/`lng`/`country`) match the client form field names
/// 1:1 so a 422 lands on the right input via AntD `Form.setFields`.
fn validate_site_input(input: SiteInput) -> Result<ValidSite, FieldErrors> {
    let mut errors = FieldErrors::new();

    let name = {
        let trimmed = input.name.trim();
        if trimmed.is_empty() {
            errors.add("name", "Cannot be empty");
            String::new()
        } else if trimmed.chars().count() > SITE_NAME_MAX {
            errors.add(
                "name",
                format!("Must be at most {SITE_NAME_MAX} characters"),
            );
            String::new()
        } else {
            trimmed.to_owned()
        }
    };

    if !(-90.0..=90.0).contains(&input.lat) {
        errors.add("lat", "Must be between -90 and 90");
    }
    if !(-180.0..=180.0).contains(&input.lng) {
        errors.add("lng", "Must be between -180 and 180");
    }

    let country = match input.country.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(c) if c.len() == 2 && c.chars().all(|ch| ch.is_ascii_alphabetic()) => {
            Some(c.to_ascii_uppercase())
        }
        Some(_) => {
            errors.add("country", "Must be a 2-letter country code");
            None
        }
    };

    if errors.is_empty() {
        Ok(ValidSite {
            name,
            lat: input.lat,
            lng: input.lng,
            country,
        })
    } else {
        Err(errors)
    }
}

/// Pack `(name, id)` and base64url-encode. The name is length-prefixed so the
/// cursor self-describes.
///
/// Layout: `[0]` `u8` name length, `[1..1+len]` raw UTF-8 name bytes,
/// `[1+len..5+len]` big-endian `i32` id.
fn encode_cursor(name: &str, id: i32) -> String {
    debug_assert!(
        name.len() <= SITE_NAME_MAX,
        "site name is {} bytes, > SITE_NAME_MAX={SITE_NAME_MAX}",
        name.len(),
    );
    let name = name.as_bytes();
    let mut buf = Vec::with_capacity(5 + name.len());
    buf.push(name.len() as u8);
    buf.extend_from_slice(name);
    buf.extend_from_slice(&id.to_be_bytes());
    URL_SAFE_NO_PAD.encode(buf)
}

/// Decode a base64url cursor into `(name, id)`. Rejects bad base64, a decoded
/// length inconsistent with the embedded name-length byte, a name length past
/// [`SITE_NAME_MAX`], and non-UTF-8 name bytes.
fn decode_cursor(s: &str) -> Result<(String, i32), AppError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| AppError::BadRequest("malformed cursor".into()))?;
    if bytes.is_empty() {
        return Err(AppError::BadRequest("malformed cursor".into()));
    }

    let name_len = bytes[0] as usize;
    if name_len > SITE_NAME_MAX || bytes.len() != 1 + name_len + 4 {
        return Err(AppError::BadRequest("malformed cursor".into()));
    }

    let name = std::str::from_utf8(&bytes[1..1 + name_len])
        .map_err(|_| AppError::BadRequest("malformed cursor".into()))?
        .to_owned();
    let id = i32::from_be_bytes(
        bytes[1 + name_len..]
            .try_into()
            .expect("4 bytes by length check above"),
    );

    Ok((name, id))
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips() {
        let (name, id) = decode_cursor(&encode_cursor("Ruhpolding", 42)).unwrap();
        assert_eq!(name, "Ruhpolding");
        assert_eq!(id, 42);
    }

    #[test]
    fn cursor_round_trips_unicode_name() {
        let (name, id) = decode_cursor(&encode_cursor("Kössen", 7)).unwrap();
        assert_eq!(name, "Kössen");
        assert_eq!(id, 7);
    }

    #[test]
    fn cursor_rejects_bad_base64() {
        assert!(matches!(
            decode_cursor("not base64!!!"),
            Err(AppError::BadRequest(_)),
        ));
    }

    #[test]
    fn cursor_rejects_length_mismatch() {
        // Header says name is 8 bytes; payload carries 4 + a 4-byte id.
        let mut buf = vec![8u8];
        buf.extend_from_slice(b"ABCD");
        buf.extend_from_slice(&1i32.to_be_bytes());
        assert!(matches!(
            decode_cursor(&URL_SAFE_NO_PAD.encode(buf)),
            Err(AppError::BadRequest(_)),
        ));
    }
}
