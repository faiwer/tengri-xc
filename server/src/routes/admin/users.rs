//! `/admin/users/*` — list and inspect users. Both endpoints require
//! the `MANAGE_USERS` bit.
//!
//! - `GET /admin/users?q=&cursor=&limit=` — keyset-paginated list,
//!   newest first by `(created_at DESC, id DESC)`. `q` matches `name`
//!   or `email` case-insensitively (`ILIKE`); empty / missing means
//!   no filter. The cursor is opaque — internally an 8-byte
//!   `(u32 created_at, i32 id)` pack rendered as base64url.
//! - `GET /admin/users/:id` — full [`UserDto`] (same shape as
//!   `/users/me`).
//!
//! Search uses `ILIKE` (no trigram index yet); the user table is
//! small enough that a Seq Scan is fine. When it isn't, the migration
//! is `CREATE EXTENSION pg_trgm` + a `gin_trgm_ops` index on
//! `name || ' ' || coalesce(email, '')`.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use crate::{
    AppError, AppState,
    auth::{Identity, require_permission},
    db::{Order, Sql},
    user::{Permissions, UserDto, fetch_user},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(list))
        .route("/admin/users/{id}", get(detail))
}

const DEFAULT_LIMIT: u32 = 25;
const MAX_LIMIT: u32 = 100;
/// Hard cap on `?q=`. Above this we 400. Avoids someone shipping a
/// 1MB pattern through the SQL parameter and pinning a worker on the
/// `ILIKE` scan.
const MAX_QUERY_LEN: usize = 128;

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
    /// Opaque cursor for the next page, or `null` on the last page.
    /// Pass it back verbatim as `?cursor=...`.
    next_cursor: Option<String>,
}

/// Trimmed projection for the table view. Profile data isn't joined —
/// the list page doesn't render it, and adding the join here would
/// double the row width for every paint.
#[derive(Debug, Serialize)]
struct ListItem {
    id: i32,
    name: String,
    login: Option<String>,
    email: Option<String>,
    permissions: i32,
    /// Unix epoch seconds (UTC). See [`UserDto`] for why we project
    /// `timestamptz` as `bigint` on the wire.
    created_at: i64,
    last_login_at: Option<i64>,
}

async fn list(
    State(state): State<AppState>,
    identity: Identity,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, AppError> {
    require_permission(&identity, Permissions::MANAGE_USERS)?;

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
    // `%` and `_` are LIKE wildcards; escape them so a query like
    // `foo%` matches the literal `foo%` in `name`/`email`, not
    // `foo<anything>`. `\` becomes `\\` first or it'd consume the
    // backslash we're about to add.
    let pattern = needle.map(|s| {
        let mut escaped = s.replace('\\', "\\\\");
        escaped = escaped.replace('%', "\\%").replace('_', "\\_");
        format!("%{escaped}%")
    });

    let cursor = q.cursor.as_deref().map(decode_cursor).transpose()?;

    let mut query = Sql::select(&[
        "id",
        "name",
        "login",
        "email",
        "permissions",
        "EXTRACT(EPOCH FROM created_at)::bigint",
        "EXTRACT(EPOCH FROM last_login_at)::bigint",
    ])
    .from("users")
    .order_by("created_at", Order::Desc)
    .order_by("id", Order::Desc)
    .limit(probe);

    if let Some((c_t, c_id)) = cursor {
        query.and_where(
            "(created_at, id) < (to_timestamp($), $)",
            (c_t as i64, c_id),
        );
    }
    if let Some(pat) = pattern.as_deref() {
        query.and_where(
            "name ILIKE $ ESCAPE '\\' OR email ILIKE $ ESCAPE '\\'",
            (pat, pat),
        );
    }

    type Row = (
        i32,
        String,
        Option<String>,
        Option<String>,
        i32,
        i64,
        Option<i64>,
    );
    let mut rows: Vec<Row> = query.fetch_all(state.pool()).await.map_err(into_internal)?;

    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.truncate(limit as usize);
    }

    let next_cursor = if has_more {
        let (id, _, _, _, _, created_at, _) = rows.last().expect("has_more implies non-empty");
        Some(encode_cursor(*created_at as u32, *id))
    } else {
        None
    };

    let items = rows
        .into_iter()
        .map(
            |(id, name, login, email, permissions, created_at, last_login_at)| ListItem {
                id,
                name,
                login,
                email,
                permissions,
                created_at,
                last_login_at,
            },
        )
        .collect();

    Ok(Json(ListResponse { items, next_cursor }))
}

async fn detail(
    State(state): State<AppState>,
    identity: Identity,
    Path(id): Path<i32>,
) -> Result<Json<UserDto>, AppError> {
    require_permission(&identity, Permissions::MANAGE_USERS)?;

    fetch_user(state.pool(), id)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

/// Pack `(created_at, id)` into 8 bytes and base64url-encode. Both
/// fields are fixed-width so no length prefix is needed; the decoder
/// rejects anything that isn't exactly 8 bytes.
fn encode_cursor(created_at: u32, id: i32) -> String {
    let mut buf = [0u8; 8];
    buf[..4].copy_from_slice(&created_at.to_be_bytes());
    buf[4..].copy_from_slice(&id.to_be_bytes());
    URL_SAFE_NO_PAD.encode(buf)
}

fn decode_cursor(s: &str) -> Result<(u32, i32), AppError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| AppError::BadRequest("malformed cursor".into()))?;
    if bytes.len() != 8 {
        return Err(AppError::BadRequest("malformed cursor".into()));
    }
    let created_at = u32::from_be_bytes(bytes[..4].try_into().expect("4 bytes by length check"));
    let id = i32::from_be_bytes(bytes[4..].try_into().expect("4 bytes by length check"));
    Ok((created_at, id))
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips() {
        let (t, id) = decode_cursor(&encode_cursor(1_777_887_122, 42)).unwrap();
        assert_eq!(t, 1_777_887_122);
        assert_eq!(id, 42);
    }

    #[test]
    fn cursor_round_trips_max_id() {
        let (t, id) = decode_cursor(&encode_cursor(u32::MAX, i32::MAX)).unwrap();
        assert_eq!(t, u32::MAX);
        assert_eq!(id, i32::MAX);
    }

    #[test]
    fn cursor_rejects_bad_base64() {
        assert!(matches!(
            decode_cursor("not base64!!!"),
            Err(AppError::BadRequest(_)),
        ));
    }

    #[test]
    fn cursor_rejects_wrong_length() {
        let short = URL_SAFE_NO_PAD.encode([0u8; 4]);
        let long = URL_SAFE_NO_PAD.encode([0u8; 12]);
        assert!(matches!(
            decode_cursor(&short),
            Err(AppError::BadRequest(_)),
        ));
        assert!(matches!(decode_cursor(&long), Err(AppError::BadRequest(_))));
    }
}
