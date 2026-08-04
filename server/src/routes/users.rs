//! `/users/*` — auth and current-user.
//!
//! - `POST /users/login`  — `{ identifier, password }` → cookie + `/users/me`
//!   body. Identifier matches `login` or `email`, case-insensitively.
//! - `POST /users/logout` — clear the cookie. Always 204.
//! - `GET  /users/me`     — current user, or `null` if anonymous. Always 200.
//! - `POST /users/me/password` — owner-self change/set password. Sets an
//!   initial `login` too when the account has none. Returns the refreshed
//!   `/users/me` body.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::Row;

use crate::{
    AppError, AppState,
    auth::{
        Claims, Identity,
        cookie::{clear_session, set_session},
        password::{self, Verified},
        token::encode_jwt,
    },
    db::Update,
    user::{
        MeDto, Permissions, UpdatePreferencesRequest, UpdateProfileRequest,
        apply_preferences_update, apply_profile_update, fetch_me, validate_preferences_update,
        validate_profile_update,
    },
    validation::FieldErrors,
};

/// Routes that set/clear the cookie inline; mounted *outside*
/// the slide middleware.
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/users/login", post(login))
        .route("/users/logout", post(logout))
}

/// Routes that read identity from extensions; mounted behind the
/// slide middleware.
pub fn session_router() -> Router<AppState> {
    Router::new()
        .route("/users/me", get(me).patch(update_me))
        .route("/users/me/password", post(change_password))
}

// -----------------------------------------------------------------
// POST /users/login
// -----------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Matched against `users.login` (case-insensitive, via the
    /// `users_login_key` functional index) or `users.email`
    /// (stored lowercased).
    pub identifier: String,
    pub password: String,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Response, AppError> {
    let identifier = req.identifier.trim();
    if identifier.is_empty() || req.password.is_empty() {
        return Err(AppError::BadRequest(
            "identifier and password are required".into(),
        ));
    }

    // Try login *and* email in one query. `users_login_key` is on
    // `LOWER(login)`; email is stored lowercased.
    let row = sqlx::query(
        "SELECT id, name, permissions, password_hash \
         FROM users \
         WHERE LOWER(login) = LOWER($1) \
            OR email        = LOWER($1) \
         LIMIT 1",
    )
    .bind(identifier)
    .fetch_optional(state.pool())
    .await
    .map_err(into_internal)?;

    // Same 401 for "no such user" and "wrong password" so
    // attackers can't tell logins apart by response.
    let Some(row) = row else {
        return Err(AppError::Unauthorized);
    };

    let user_id: i32 = row.try_get("id").map_err(sqlx_to_internal)?;
    let name: String = row.try_get("name").map_err(sqlx_to_internal)?;
    let permissions_bits: i32 = row.try_get("permissions").map_err(sqlx_to_internal)?;
    let stored_hash: Option<String> = row.try_get("password_hash").map_err(sqlx_to_internal)?;

    // OAuth-only account with no password set.
    let Some(stored_hash) = stored_hash else {
        return Err(AppError::Unauthorized);
    };

    let verified = match password::verify(&req.password, &stored_hash) {
        Ok(Some(v)) => v,
        Ok(None) => return Err(AppError::Unauthorized),
        Err(e) => {
            // Corrupt stored hash. Fail closed.
            tracing::error!(user_id, error = %e, "password verify failed on stored hash");
            return Err(AppError::Unauthorized);
        }
    };

    let permissions = Permissions::from_bits_retain(permissions_bits);
    // Banned/soft-disabled. Same 401 as wrong password.
    if !permissions.contains(Permissions::CAN_AUTHORIZE) {
        return Err(AppError::Unauthorized);
    }

    // Persist the rehash *before* minting so a panic between
    // verify and write doesn't leave the row stuck on phpass.
    // A failed update is fine — we'll retry next login.
    if let Verified {
        rehashed: Some(new_hash),
    } = &verified
    {
        if let Err(e) = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(new_hash)
            .bind(user_id)
            .execute(state.pool())
            .await
        {
            tracing::error!(user_id, error = %e, "rehash write failed; will retry next login");
        } else {
            tracing::info!(user_id, "rehashed phpass → argon2");
        }
    }

    if let Err(e) = sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(state.pool())
        .await
    {
        tracing::warn!(user_id, error = %e, "failed to update last_login_at");
    }

    let claims = Claims::new(user_id, name, permissions, Utc::now().timestamp());
    let jwt = encode_jwt(&claims, state.jwt_encoding_key()).map_err(jwt_to_internal)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&set_session(&jwt, state.https())).map_err(into_internal)?,
    );

    let body = fetch_me(state.pool(), user_id)
        .await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("user {user_id} vanished mid-login")))?;
    Ok((StatusCode::OK, headers, Json(body)).into_response())
}

// -----------------------------------------------------------------
// POST /users/logout
// -----------------------------------------------------------------

/// 204 even if there was no session — idempotent, and avoids
/// noisy 401s when the client logs out twice.
async fn logout(State(state): State<AppState>) -> Result<Response, AppError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&clear_session(state.https())).map_err(into_internal)?,
    );
    Ok((StatusCode::NO_CONTENT, headers).into_response())
}

// -----------------------------------------------------------------
// GET /users/me
// -----------------------------------------------------------------

/// `null` for anonymous, current user otherwise. Always 200 —
/// "nobody" is a valid answer here, and 401 would just spam the
/// browser console with red errors on every anon SPA boot.
async fn me(
    State(state): State<AppState>,
    identity: Option<Identity>,
) -> Result<Json<Option<MeDto>>, AppError> {
    let Some(identity) = identity else {
        return Ok(Json(None));
    };
    // Row missing = user hard-deleted between the last slide and
    // now. Treat as anonymous; the next request slides cleanly.
    Ok(Json(fetch_me(state.pool(), identity.user_id).await?))
}

// -----------------------------------------------------------------
// PATCH /users/me
// -----------------------------------------------------------------

/// Owner-edit envelope. Each top-level block is optional and applied
/// independently — the FE form for preferences sends only `preferences`,
/// a future profile form sends only `profile`, and a "save everything"
/// flow can send both. Empty body = 400 (no-op PATCH is a misuse).
///
/// Admin endpoints don't share this struct: the admin form has its own
/// envelope (with `permissions`, etc.) but reuses the same per-section
/// validators and appliers from `user::profile` / `user::preferences`.
#[derive(Debug, Deserialize)]
pub struct UpdateMeRequest {
    #[serde(default)]
    pub profile: Option<UpdateProfileRequest>,
    #[serde(default)]
    pub preferences: Option<UpdatePreferencesRequest>,
}

async fn update_me(
    State(state): State<AppState>,
    identity: Identity,
    Json(req): Json<UpdateMeRequest>,
) -> Result<Json<MeDto>, AppError> {
    if req.profile.is_none() && req.preferences.is_none() {
        return Err(AppError::BadRequest(
            "PATCH body must include at least one of: profile, preferences".into(),
        ));
    }

    // Two-pass: validate everything first, accumulate per-field errors
    // under namespaced keys, *then* apply. A single bad field shouldn't
    // half-write the request.
    let mut errors = FieldErrors::new();
    let mut profile_update = None;
    if let Some(input) = req.profile {
        match validate_profile_update(input) {
            Ok(u) => profile_update = Some(u),
            Err(field_errors) => errors.merge_prefixed("profile", field_errors),
        }
    }
    let mut preferences_update = None;
    if let Some(input) = req.preferences {
        match validate_preferences_update(input) {
            Ok(u) => preferences_update = Some(u),
            Err(field_errors) => errors.merge_prefixed("preferences", field_errors),
        }
    }
    errors.into_result()?;

    // Single transaction so a profile-write that succeeds is rolled
    // back if the preferences-write trips on a constraint (or vice
    // versa). Failures here are infra-level (DB went away mid-request);
    // user-input failures already turned into 422 above.
    let mut tx = state
        .pool()
        .begin()
        .await
        .map_err(|e| AppError::Internal(anyhow::Error::new(e)))?;
    if let Some(u) = profile_update {
        apply_profile_update(&mut tx, identity.user_id, &u).await?;
    }
    if let Some(u) = preferences_update {
        apply_preferences_update(&mut tx, identity.user_id, &u).await?;
    }
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(anyhow::Error::new(e)))?;

    let body = fetch_me(state.pool(), identity.user_id)
        .await?
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "user {} vanished mid-update",
                identity.user_id
            ))
        })?;
    Ok(Json(body))
}

/// Change (or, for a password-less account, set) the caller's password. `login`
/// is honored only when the account has none yet — a set-once initial login;
/// once a login exists the client sends nothing and any value here is ignored.
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    #[serde(default)]
    login: Option<String>,
    #[serde(default)]
    current_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<AppState>,
    identity: Identity,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<MeDto>, AppError> {
    let user_id = identity.user_id;

    let row = sqlx::query("SELECT login, email, password_hash FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(state.pool())
        .await
        .map_err(into_internal)?
        .ok_or(AppError::NotFound)?;
    let stored_login: Option<String> = row.try_get("login").map_err(sqlx_to_internal)?;
    let stored_email: Option<String> = row.try_get("email").map_err(sqlx_to_internal)?;
    let stored_hash: Option<String> = row.try_get("password_hash").map_err(sqlx_to_internal)?;

    // Stage one: format + credential checks. The login-availability query is
    // deliberately *not* here — it runs last, so a caller who fails these can't
    // use this endpoint to enumerate taken logins.
    let mut errors = FieldErrors::new();

    if let Some(msg) = weak_password(&req.new_password) {
        errors.add("new_password", msg);
    }

    // A password-less account (OAuth / import) sets its first password here
    // with no current-password check. Otherwise verify it.
    if let Some(hash) = stored_hash.as_deref() {
        if req.current_password.is_empty() {
            errors.add("current_password", "Enter your current password");
        } else {
            match password::verify(&req.current_password, hash) {
                Ok(Some(_)) => {}
                Ok(None) => errors.add("current_password", "Incorrect password"),
                Err(e) => return Err(AppError::Internal(e.into())),
            }
        }
    }

    // Login is set-once: editable only while NULL.
    let new_login = if stored_login.is_none() {
        req.login
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    } else {
        None
    };

    // The account must be able to sign in afterward.
    if stored_login.is_none() && new_login.is_none() && stored_email.is_none() {
        errors.add("login", "Set a login so you can sign in");
    }

    errors.into_result()?;

    // Runs last, and only when setting an initial login. Case-insensitive,
    // matching the `users_login_key` functional index.
    if let Some(login) = new_login.as_deref() {
        let taken: Option<i32> =
            sqlx::query_scalar("SELECT id FROM users WHERE LOWER(login) = LOWER($1) AND id <> $2")
                .bind(login)
                .bind(user_id)
                .fetch_optional(state.pool())
                .await
                .map_err(into_internal)?;
        if taken.is_some() {
            let mut errors = FieldErrors::new();
            errors.add("login", "Already taken");
            errors.into_result()?;
        }
    }

    let hash =
        password::hash_argon2(&req.new_password).map_err(|e| AppError::Internal(e.into()))?;

    let mut q = Update::new("users");
    q.set("password_hash", hash);
    if let Some(login) = new_login {
        q.set("login", login);
    }
    q.and_where("id = $", (user_id,));
    q.execute(state.pool()).await.map_err(into_internal)?;

    let body = fetch_me(state.pool(), user_id).await?.ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "user {user_id} vanished mid-password-change"
        ))
    })?;
    Ok(Json(body))
}

/// `None` when `password` meets the policy (>= 8 chars, at least one letter and
/// one digit), otherwise the message to surface on the field. Mirrored
/// client-side in the Authorization form.
fn weak_password(password: &str) -> Option<&'static str> {
    if password.chars().count() < 8 {
        return Some("At least 8 characters");
    }
    let has_letter = password.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_letter || !has_digit {
        return Some("Must include a letter and a digit");
    }
    None
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
fn sqlx_to_internal(e: sqlx::Error) -> AppError {
    AppError::Internal(anyhow::Error::new(e))
}
fn jwt_to_internal(e: jsonwebtoken::errors::Error) -> AppError {
    AppError::Internal(anyhow::Error::new(e))
}
