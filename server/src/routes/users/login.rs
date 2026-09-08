use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use sqlx::PgPool;

use super::{into_internal, jwt_to_internal};
use crate::{
    AppError, AppState,
    auth::{
        password::{self, Verified},
        token::mint_session_cookie,
    },
    user::{Permissions, fetch_me},
};

/// `error` code on login's unconfirmed-address 403. The SPA keys its "check
/// your inbox" copy off this string, so the two have to move together.
const EMAIL_UNCONFIRMED: &str = "email_unconfirmed";

/// `error` code on login's cleared-`CAN_AUTHORIZE` 403. Same contract with the
/// SPA as [`EMAIL_UNCONFIRMED`].
const ACCOUNT_DISABLED: &str = "account_disabled";

#[derive(Debug, Deserialize)]
pub(super) struct LoginRequest {
    /// Matched against `users.login` (case-insensitive, via the
    /// `users_login_key` functional index) or `users.email`
    /// (stored lowercased).
    identifier: String,
    password: String,
}

pub(super) async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Response, AppError> {
    let identifier = req.identifier.trim();
    if identifier.is_empty() || req.password.is_empty() {
        return Err(AppError::BadRequest(
            "identifier and password are required".into(),
        ));
    }

    // Same 401 as a wrong password so attackers can't tell logins apart.
    let Some(creds) = find_credentials(state.pool(), identifier).await? else {
        return Err(AppError::Unauthorized);
    };

    let verified = authenticate(&creds, &req.password)?;
    record_login(state.pool(), creds.id, &verified).await;

    let cookie = mint_session_cookie(
        creds.id,
        creds.name,
        Permissions::from_bits_retain(creds.permissions),
        state.jwt_encoding_key(),
        state.https(),
    )
    .map_err(jwt_to_internal)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(into_internal)?,
    );

    let body = fetch_me(state.pool(), creds.id).await?.ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!("user {} vanished mid-login", creds.id))
    })?;
    Ok((StatusCode::OK, headers, Json(body)).into_response())
}

#[derive(Debug, sqlx::FromRow)]
struct Credentials {
    id: i32,
    name: String,
    permissions: i32,
    /// `None` on an OAuth-only account, which therefore can't log in here.
    password_hash: Option<String>,
    /// Registration parked an address and nobody has clicked the link yet.
    needs_confirmation: bool,
}

/// The one row `identifier` can name, matching either column in a single
/// query. `users_login_key` is on `LOWER(login)`; email is stored lowercased.
async fn find_credentials(
    pool: &PgPool,
    identifier: &str,
) -> Result<Option<Credentials>, AppError> {
    sqlx::query_as::<_, Credentials>(
        "SELECT id, name, permissions, password_hash, \
                email IS NULL AND pending_email IS NOT NULL AS needs_confirmation \
         FROM users \
         WHERE LOWER(login) = LOWER($1) \
            OR email        = LOWER($1) \
         LIMIT 1",
    )
    .bind(identifier)
    .fetch_optional(pool)
    .await
    .map_err(into_internal)
}

/// Every way a password login can be refused, in one place.
fn authenticate(creds: &Credentials, password: &str) -> Result<Verified, AppError> {
    let Some(stored_hash) = creds.password_hash.as_deref() else {
        return Err(AppError::Unauthorized);
    };

    let verified = match password::verify(password, stored_hash) {
        Ok(Some(v)) => v,
        Ok(None) => return Err(AppError::Unauthorized),
        Err(e) => {
            // Corrupt stored hash. Fail closed.
            let user_id = creds.id;
            tracing::error!(user_id, error = %e, "password verify failed on stored hash");
            return Err(AppError::Unauthorized);
        }
    };

    // Banned/soft-disabled. Named rather than folded into the 401s above for
    // the same reason as the confirmation gate below: the password checked out,
    // so "your account is disabled" tells the caller nothing they couldn't
    // already confirm, and leaving them to guess at "wrong login or password"
    // sends them round the password-reset loop forever.
    if !Permissions::from_bits_retain(creds.permissions).contains(Permissions::CAN_AUTHORIZE) {
        return Err(AppError::ForbiddenReason {
            code: ACCOUNT_DISABLED,
            message: "This account is disabled — contact the site administrator".into(),
        });
    }

    // Registered but never clicked the confirmation link. 403 with a reason,
    // not the 401s above: the password was already correct, so telling them to
    // check their inbox gives nothing away.
    if creds.needs_confirmation {
        return Err(AppError::ForbiddenReason {
            code: EMAIL_UNCONFIRMED,
            message: "Confirm your email address before signing in — check your inbox for the link"
                .into(),
        });
    }

    Ok(verified)
}

/// Bookkeeping for a login that already succeeded. Neither write is worth
/// failing the request over, so both only log.
async fn record_login(pool: &PgPool, user_id: i32, verified: &Verified) {
    // Runs before the cookie is minted so a panic in between doesn't leave the
    // row stuck on phpass. We'll retry next login if it fails.
    if let Verified {
        rehashed: Some(new_hash),
    } = verified
    {
        // Replace Leonardo's hash with Tengri XC argon2 hash
        if let Err(e) = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(new_hash)
            .bind(user_id)
            .execute(pool)
            .await
        {
            tracing::error!(user_id, error = %e, "rehash write failed; will retry next login");
        } else {
            tracing::info!(user_id, "rehashed phpass → argon2");
        }
    }

    if let Err(e) = sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await
    {
        tracing::warn!(user_id, error = %e, "failed to update last_login_at");
    }
}
