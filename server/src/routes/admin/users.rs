//! `/admin/users/*` — list and inspect users. Both endpoints require the
//! `MANAGE_USERS` bit.
//!
//! - `GET /admin/users?q=&cursor=&limit=` — keyset-paginated list. Sort: admins
//!   first, then newest first — `(is_admin DESC, created_at DESC, id DESC)`.
//!   "Admin" is "any permission bit other than [`Permissions::CAN_AUTHORIZE`]",
//!   matching the client's `isAdminBits`. `q` matches `name`, `login`, or
//!   `email` case-insensitively (`ILIKE`); empty / missing means no filter. The
//!   cursor is opaque — internally `[is_admin][created_at][id]` = 1+4+4 bytes
//!   rendered as base64url.
//! - `GET /admin/users/:id` — full [`UserDto`] (same shape as `/users/me`).
//!
//! Search uses `ILIKE` (no trigram index yet); the user table is small enough
//! that a Seq Scan is fine. When it isn't, the migration is `CREATE EXTENSION
//! pg_trgm` + a `gin_trgm_ops` index on `name || ' ' || coalesce(login, '') ||
//! ' ' || coalesce(email, '')`.

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

/// SQL expression that mirrors the client's `isAdminBits` — true when
/// any permission bit other than [`Permissions::CAN_AUTHORIZE`] is
/// set. Forward-compatible: any future capability bit added to
/// [`Permissions`] folds in without touching this string.
const IS_ADMIN_SQL: &str = "(u.permissions & ~1) <> 0";

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

/// Trimmed projection for the table view. `country` is the only profile-side
/// field we pull (one optional `text`, ~3 bytes); the rest of the profile stays
/// off the list query.
#[derive(Debug, Serialize)]
struct ListItem {
    id: i32,
    name: String,
    login: Option<String>,
    email: Option<String>,
    permissions: i32,
    /// ISO 3166-1 alpha-2, from the user's profile. `None` when the
    /// user has no profile row or hasn't set a country.
    country: Option<String>,
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
        "u.id",
        "u.name",
        "u.login",
        "u.email",
        "u.permissions",
        "p.country",
        "EXTRACT(EPOCH FROM u.created_at)::bigint",
        "EXTRACT(EPOCH FROM u.last_login_at)::bigint",
    ])
    .from("users u")
    .left_join("user_profiles p", "p.user_id = u.id")
    .order_by(IS_ADMIN_SQL, Order::Desc)
    .order_by("u.created_at", Order::Desc)
    .order_by("u.id", Order::Desc)
    .limit(probe);

    // Row-comparison against the same `(is_admin, created_at, id)`
    // tuple that defines the sort order. With all three DESC,
    // `< cursor` picks the rows that come *after* the cursor row.
    if let Some((c_admin, c_t, c_id)) = cursor {
        query.and_where(
            "((u.permissions & ~1) <> 0, u.created_at, u.id) < ($, to_timestamp($), $)",
            (c_admin, c_t as i64, c_id),
        );
    }
    if let Some(pat) = pattern.as_deref() {
        query.and_where(
            "u.name ILIKE $ ESCAPE '\\' OR u.login ILIKE $ ESCAPE '\\' OR u.email ILIKE $ ESCAPE '\\'",
            (pat, pat, pat),
        );
    }

    type Row = (
        i32,
        String,
        Option<String>,
        Option<String>,
        i32,
        Option<String>,
        i64,
        Option<i64>,
    );
    let mut rows: Vec<Row> = query.fetch_all(state.pool()).await.map_err(into_internal)?;

    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.truncate(limit as usize);
    }

    let next_cursor = if has_more {
        let (id, _, _, _, permissions, _, created_at, _) =
            rows.last().expect("has_more implies non-empty");
        Some(encode_cursor(
            is_admin(*permissions),
            *created_at as u32,
            *id,
        ))
    } else {
        None
    };

    let items = rows
        .into_iter()
        .map(
            |(id, name, login, email, permissions, country, created_at, last_login_at)| ListItem {
                id,
                name,
                login,
                email,
                permissions,
                country,
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

/// Pack `(is_admin, created_at, id)` into 9 bytes and base64url-encode.
/// All fields are fixed-width so no length prefix is needed; the
/// decoder rejects anything that isn't exactly 9 bytes.
fn encode_cursor(is_admin: bool, created_at: u32, id: i32) -> String {
    let mut buf = [0u8; 9];
    buf[0] = u8::from(is_admin);
    buf[1..5].copy_from_slice(&created_at.to_be_bytes());
    buf[5..].copy_from_slice(&id.to_be_bytes());
    URL_SAFE_NO_PAD.encode(buf)
}

fn decode_cursor(s: &str) -> Result<(bool, u32, i32), AppError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| AppError::BadRequest("malformed cursor".into()))?;
    if bytes.len() != 9 {
        return Err(AppError::BadRequest("malformed cursor".into()));
    }

    // Reject anything other than 0 / 1 so cursors round-trip exactly
    // and a tampered byte can't shift sort behaviour.
    let is_admin = match bytes[0] {
        0 => false,
        1 => true,
        _ => return Err(AppError::BadRequest("malformed cursor".into())),
    };
    let created_at = u32::from_be_bytes(bytes[1..5].try_into().expect("4 bytes by length check"));
    let id = i32::from_be_bytes(bytes[5..].try_into().expect("4 bytes by length check"));
    Ok((is_admin, created_at, id))
}

/// Mirrors the client's `isAdminBits`: a user is "admin" iff any
/// permission bit beyond [`Permissions::CAN_AUTHORIZE`] is set.
fn is_admin(permissions: i32) -> bool {
    !Permissions::from_bits_retain(permissions)
        .difference(Permissions::CAN_AUTHORIZE)
        .is_empty()
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips_non_admin() {
        let (a, t, id) = decode_cursor(&encode_cursor(false, 1_777_887_122, 42)).unwrap();
        assert!(!a);
        assert_eq!(t, 1_777_887_122);
        assert_eq!(id, 42);
    }

    #[test]
    fn cursor_round_trips_admin() {
        let (a, t, id) = decode_cursor(&encode_cursor(true, 1_777_887_122, 42)).unwrap();
        assert!(a);
        assert_eq!(t, 1_777_887_122);
        assert_eq!(id, 42);
    }

    #[test]
    fn cursor_round_trips_max_id() {
        let (a, t, id) = decode_cursor(&encode_cursor(true, u32::MAX, i32::MAX)).unwrap();
        assert!(a);
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

    #[test]
    fn is_admin_matches_client() {
        // Mirror the FE rule: CAN_AUTHORIZE alone is *not* admin; any
        // other bit (or combination) is.
        assert!(!is_admin(0));
        assert!(!is_admin(Permissions::CAN_AUTHORIZE.bits()));
        assert!(is_admin(Permissions::MANAGE_USERS.bits()));
        assert!(is_admin(
            (Permissions::CAN_AUTHORIZE | Permissions::MANAGE_USERS).bits(),
        ));
    }
}
