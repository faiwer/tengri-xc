//! `/users/*` — auth and current-user.
//!
//! - `POST /users/login`  — `{ identifier, password }` → cookie +
//!   `/users/me` body. Identifier matches `login` or `email`,
//!   case-insensitively.
//! - `POST /users/logout` — clear the cookie. Always 204.
//! - `GET  /users/me`     — current user, or `null` if anonymous.
//!   Always 200.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{
    AppError, AppState,
    auth::{
        Claims, Identity,
        cookie::{clear_session, set_session},
        password::{self, Verified},
        token::encode_jwt,
    },
    user::{Permissions, UserSex, UserSource},
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
    Router::new().route("/users/me", get(me))
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

    let body = fetch_me(state.pool(), user_id).await?;
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

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: i32,
    pub name: String,
    pub login: Option<String>,
    pub email: Option<String>,
    pub source: UserSource,
    /// Raw bits. Frontend uses `bit & N` checks, no enum needed.
    pub permissions: i32,
    /// Unix epoch seconds (UTC). The DB stores `timestamptz`; we
    /// project it as `bigint` so the wire stays numeric and the
    /// client can do `new Date(seconds * 1000)` without parsing.
    pub email_verified_at: Option<i64>,
    pub last_login_at: Option<i64>,
    pub created_at: i64,
    pub profile: Option<MeProfile>,
}

#[derive(Debug, Serialize)]
pub struct MeProfile {
    pub civl_id: Option<i32>,
    pub country: Option<String>,
    pub sex: Option<UserSex>,
}

/// `null` for anonymous, current user otherwise. Always 200 —
/// "nobody" is a valid answer here, and 401 would just spam the
/// browser console with red errors on every anon SPA boot.
async fn me(
    State(state): State<AppState>,
    identity: Option<Identity>,
) -> Result<Json<Option<MeResponse>>, AppError> {
    let Some(identity) = identity else {
        return Ok(Json(None));
    };
    // Row missing = user hard-deleted between the last slide and
    // now. Treat as anonymous; the next request slides cleanly.
    Ok(Json(fetch_me_opt(state.pool(), identity.user_id).await?))
}

/// User row + profile join. `None` if the row is gone.
async fn fetch_me_opt(pool: &sqlx::PgPool, user_id: i32) -> Result<Option<MeResponse>, AppError> {
    let row = sqlx::query(
        "SELECT \
            u.id, u.name, u.login, u.email, u.source, u.permissions, \
            EXTRACT(EPOCH FROM u.email_verified_at)::bigint AS email_verified_at, \
            EXTRACT(EPOCH FROM u.last_login_at)::bigint     AS last_login_at, \
            EXTRACT(EPOCH FROM u.created_at)::bigint        AS created_at, \
            p.civl_id, p.country, p.sex \
         FROM users u \
         LEFT JOIN user_profiles p ON p.user_id = u.id \
         WHERE u.id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(into_internal)?;

    let Some(row) = row else { return Ok(None) };

    let civl_id: Option<i32> = row.try_get("civl_id").map_err(sqlx_to_internal)?;
    let country: Option<String> = row.try_get("country").map_err(sqlx_to_internal)?;
    let sex: Option<UserSex> = row.try_get("sex").map_err(sqlx_to_internal)?;

    // `null` for "no profile data" (whether the row was missing
    // or had all-NULL columns); `Some` if any field is populated.
    let profile = if civl_id.is_some() || country.is_some() || sex.is_some() {
        Some(MeProfile {
            civl_id,
            country,
            sex,
        })
    } else {
        None
    };

    Ok(Some(MeResponse {
        id: row.try_get("id").map_err(sqlx_to_internal)?,
        name: row.try_get("name").map_err(sqlx_to_internal)?,
        login: row.try_get("login").map_err(sqlx_to_internal)?,
        email: row.try_get("email").map_err(sqlx_to_internal)?,
        source: row.try_get("source").map_err(sqlx_to_internal)?,
        permissions: row.try_get("permissions").map_err(sqlx_to_internal)?,
        email_verified_at: row.try_get("email_verified_at").map_err(sqlx_to_internal)?,
        last_login_at: row.try_get("last_login_at").map_err(sqlx_to_internal)?,
        created_at: row.try_get("created_at").map_err(sqlx_to_internal)?,
        profile,
    }))
}

/// Login-side wrapper. The row must exist — we just SELECTed it.
/// A missing row here means it was deleted between two queries
/// in this handler; surface as 500.
async fn fetch_me(pool: &sqlx::PgPool, user_id: i32) -> Result<MeResponse, AppError> {
    fetch_me_opt(pool, user_id)
        .await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("user {user_id} vanished mid-login")))
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
