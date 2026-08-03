//! `/admin/users/*` — list, inspect, create, and edit users. Every
//! endpoint requires the `MANAGE_USERS` bit.
//!
//! - `GET /admin/users?q=&cursor=&limit=` — keyset-paginated list. Sort: admins
//!   first, then newest first — `(is_admin DESC, created_at DESC, id DESC)`.
//!   "Admin" is "any permission bit other than [`Permissions::CAN_AUTHORIZE`]",
//!   matching the client's `isAdminBits`. `q` matches `name`, `login`, or
//!   `email` case-insensitively (`ILIKE`); empty / missing means no filter. The
//!   cursor is opaque — internally `[is_admin][created_at][id]` = 1+4+4 bytes
//!   rendered as base64url.
//! - `GET /admin/users/:id` — full [`UserDto`] (same shape as `/users/me`).
//! - `POST /admin/users` — create an internal user; returns the full
//!   [`UserDto`]. The form always submits every field, so scalar columns are a
//!   full write; `password` is optional (an account can be OAuth/import-only).
//! - `PATCH /admin/users/:id` — edit an existing user; returns the full
//!   [`UserDto`]. Scalars are a full replace; an empty `password` leaves the
//!   stored hash untouched.
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
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    AppError, AppState,
    auth::{Identity, password, require_permission},
    db::{Order, Sql, Update, like_contains},
    user::{
        CreateUser, CreateUserPassword, Permissions, ProfileUpdate, UpdateProfileRequest, UserDto,
        UserSex, UserSource, apply_profile_update, create_user, fetch_user,
        validate_profile_update,
    },
    validation::FieldErrors,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(list).post(create))
        .route("/admin/users/{id}", get(detail).patch(update))
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
#[derive(Debug, Serialize, sqlx::FromRow)]
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
    let pattern = needle.map(like_contains);

    let cursor = q.cursor.as_deref().map(decode_cursor).transpose()?;

    let mut query = Sql::select(&[
        "u.id",
        "u.name",
        "u.login",
        "u.email",
        "u.permissions",
        "p.country",
        "EXTRACT(EPOCH FROM u.created_at)::bigint AS created_at",
        "EXTRACT(EPOCH FROM u.last_login_at)::bigint AS last_login_at",
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

    let mut items: Vec<ListItem> = query.fetch_all(state.pool()).await.map_err(into_internal)?;

    let has_more = items.len() > limit as usize;
    if has_more {
        items.truncate(limit as usize);
    }

    let next_cursor = if has_more {
        let last = items.last().expect("has_more implies non-empty");
        Some(encode_cursor(
            is_admin(last.permissions),
            last.created_at as u32,
            last.id,
        ))
    } else {
        None
    };

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

/// Create + edit share this body. The form always submits every field, so the
/// scalar columns are a full write rather than a partial patch. `password` is
/// the one exception — empty / absent means "don't set / don't change".
#[derive(Debug, Deserialize)]
struct UserInput {
    name: String,
    #[serde(default)]
    login: Option<String>,
    #[serde(default)]
    email: Option<String>,
    /// Drives `email_verified_at`: `true` marks the address verified, `false`
    /// clears the mark. See the handlers for how an existing timestamp is
    /// preserved on edit.
    #[serde(default)]
    email_verified: bool,
    /// Raw `Permissions` bitfield. Unknown bits are rejected.
    permissions: i32,
    /// Plaintext password to (re)set. Empty string is treated as absent.
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    profile: ProfileInput,
}

/// Profile section of [`UserInput`]. Full-replace: a field present with a
/// value sets it, `null` / absent clears it.
#[derive(Debug, Default, Deserialize)]
struct ProfileInput {
    #[serde(default)]
    civl_id: Option<i32>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    sex: Option<UserSex>,
}

/// Validated + normalised [`UserInput`]: `name` trimmed, `login` trimmed
/// (casing preserved), `email` trimmed + lowercased, blanks collapsed to
/// `None`, `permissions` masked to known bits, profile run through the shared
/// profile validator.
struct ValidUser {
    name: String,
    login: Option<String>,
    email: Option<String>,
    email_verified: bool,
    permissions: i32,
    password: Option<String>,
    profile: ProfileUpdate,
}

async fn create(
    State(state): State<AppState>,
    identity: Identity,
    Json(input): Json<UserInput>,
) -> Result<Json<UserDto>, AppError> {
    require_permission(&identity, Permissions::MANAGE_USERS)?;

    let valid = validate_user(input)?;

    // Uniqueness pre-check for clean per-field 422s; the unique indexes
    // (`users_name_key` / `users_login_key` / `users_email_key`) remain the
    // backstop for a race between this check and the insert.
    let mut errors = FieldErrors::new();
    check_unique(state.pool(), &valid, None, &mut errors).await?;
    if valid.password.is_some() && valid.login.is_none() && valid.email.is_none() {
        errors.add("password", "Set a login or email so the user can log in");
    }
    errors.into_result()?;

    let created = create_user(
        state.pool(),
        CreateUser {
            id: None,
            name: valid.name,
            login: valid.login.clone(),
            email: valid.email.clone(),
            password: valid.password.map(CreateUserPassword::Plaintext),
            permissions: valid.permissions,
            source: UserSource::Internal,
            // A brand-new account's address starts verified only if the admin
            // ticked the box *and* actually set an email.
            email_verified_at: (valid.email_verified && valid.email.is_some()).then(Utc::now),
            last_login_at: None,
            created_at: None,
        },
    )
    .await
    .map_err(AppError::Internal)?;

    // Profile lives in a separate table; write it (best-effort atomic via its
    // own transaction). Reuses the shared upsert so a missing profile row is
    // created.
    let mut tx = state.pool().begin().await.map_err(into_internal)?;
    apply_profile_update(&mut tx, created.id, &valid.profile).await?;
    tx.commit().await.map_err(into_internal)?;

    fetch_user(state.pool(), created.id)
        .await?
        .map(Json)
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!("user {} vanished after create", created.id))
        })
}

async fn update(
    State(state): State<AppState>,
    identity: Identity,
    Path(id): Path<i32>,
    Json(input): Json<UserInput>,
) -> Result<Json<UserDto>, AppError> {
    require_permission(&identity, Permissions::MANAGE_USERS)?;

    let valid = validate_user(input)?;

    // 404 before any write, and gives us the current `email_verified_at` so a
    // re-save doesn't churn an already-verified timestamp.
    let current = fetch_user(state.pool(), id)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut errors = FieldErrors::new();
    check_unique(state.pool(), &valid, Some(id), &mut errors).await?;

    let password_hash = match valid.password.as_deref() {
        Some(pw) => {
            if valid.login.is_none() && valid.email.is_none() {
                errors.add("password", "Set a login or email so the user can log in");
                None
            } else {
                Some(password::hash_argon2(pw).map_err(|e| AppError::Internal(e.into()))?)
            }
        }
        None => None,
    };
    errors.into_result()?;

    let mut tx = state.pool().begin().await.map_err(into_internal)?;

    let mut q = Update::new("users");
    q.set("name", valid.name);
    q.set("login", valid.login);
    q.set("email", valid.email);
    q.set("permissions", valid.permissions);
    if let Some(hash) = password_hash {
        q.set("password_hash", hash);
    }
    // Only touch `email_verified_at` when the flag actually flips, so an
    // unchanged "verified" checkbox preserves the original timestamp.
    match (valid.email_verified, current.email_verified_at.is_some()) {
        (true, false) => {
            q.set("email_verified_at", Some(Utc::now()));
        }
        (false, true) => {
            q.set("email_verified_at", None::<chrono::DateTime<Utc>>);
        }
        _ => {}
    }
    q.and_where("id = $", (id,));
    q.execute_tx(&mut tx).await.map_err(into_internal)?;

    apply_profile_update(&mut tx, id, &valid.profile).await?;
    tx.commit().await.map_err(into_internal)?;

    fetch_user(state.pool(), id)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("user {id} vanished after update")))
}

/// Validate + normalise the shared body. Field keys (`name`, `login`, `email`,
/// `permissions`, `profile.country`, …) match the client form field names 1:1
/// so a 422 lands on the right input via `Form.setFields`.
fn validate_user(input: UserInput) -> Result<ValidUser, AppError> {
    let mut errors = FieldErrors::new();

    let name = {
        let trimmed = input.name.trim();
        if trimmed.is_empty() {
            errors.add("name", "Cannot be empty");
        }
        trimmed.to_owned()
    };

    let login = blank_to_none(input.login);

    let email = match blank_to_none(input.email) {
        None => None,
        Some(raw) => {
            let lowered = raw.to_ascii_lowercase();
            if looks_like_email(&lowered) {
                Some(lowered)
            } else {
                errors.add("email", "Enter a valid email address");
                None
            }
        }
    };

    let permissions = input.permissions;
    if permissions < 0 || (permissions & !Permissions::all().bits()) != 0 {
        errors.add("permissions", "Unknown permission bits");
    }

    let profile = match validate_profile_update(UpdateProfileRequest {
        civl_id: Some(input.profile.civl_id),
        country: Some(input.profile.country),
        sex: Some(input.profile.sex),
    }) {
        Ok(update) => update,
        Err(field_errors) => {
            errors.merge_prefixed("profile", field_errors);
            ProfileUpdate::default()
        }
    };

    let password = input.password.filter(|p| !p.is_empty());

    errors.into_result()?;

    Ok(ValidUser {
        name,
        login,
        email,
        email_verified: input.email_verified,
        permissions,
        password,
        profile,
    })
}

/// Add a `name` / `login` / `email` field error when the value is already
/// taken by another row. `exclude` is the row being edited (skipped on PATCH).
/// Name and login fold case (`users_name_key` / `users_login_key` are on
/// `LOWER(...)`); email is stored lowercased so a plain `=` matches the index.
async fn check_unique(
    pool: &sqlx::PgPool,
    valid: &ValidUser,
    exclude: Option<i32>,
    errors: &mut FieldErrors,
) -> Result<(), AppError> {
    let exclude = exclude.unwrap_or(0);

    // (form field, `$1` predicate, value). `name` is mandatory; `login` /
    // `email` are skipped when unset. Name and login fold case; email is
    // already stored lowercased so a plain `=` matches its index. The
    // predicates are static literals — no user input reaches the SQL text.
    let mut checks: Vec<(&str, &str, &str)> =
        vec![("name", "LOWER(name) = LOWER($1)", valid.name.as_str())];
    if let Some(login) = valid.login.as_deref() {
        checks.push(("login", "LOWER(login) = LOWER($1)", login));
    }
    if let Some(email) = valid.email.as_deref() {
        checks.push(("email", "email = $1", email));
    }

    for (field, predicate, value) in checks {
        let taken: Option<i32> = sqlx::query_scalar(&format!(
            "SELECT id FROM users WHERE {predicate} AND id <> $2"
        ))
        .bind(value)
        .bind(exclude)
        .fetch_optional(pool)
        .await
        .map_err(into_internal)?;
        if taken.is_some() {
            errors.add(field, "Already taken");
        }
    }
    Ok(())
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// Deliberately loose: exactly one `@`, non-empty local + domain, no spaces.
/// Real validity is confirmed by a verification mail, not a regex — this only
/// catches obvious typos.
fn looks_like_email(value: &str) -> bool {
    let mut parts = value.split('@');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(local), Some(domain), None) => {
            !local.is_empty() && !domain.is_empty() && !value.contains(char::is_whitespace)
        }
        _ => false,
    }
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
