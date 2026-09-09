//! `POST /users/reset-password/confirm` — spend a reset link on a new password
//! and sign the user in, the way the confirmation link does.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::{DateTime, Utc};
use jsonwebtoken::errors::ErrorKind;
use serde::Deserialize;
use sqlx::PgPool;

use super::into_internal;
use crate::{
    AppError, AppState,
    auth::{password, token::mint_session_cookie_at, verify_reset_token},
    db::Update,
    user::{MeDto, Permissions, fetch_me, weak_password},
    validation::FieldErrors,
};

/// `error` code for a link that has expired. The SPA offers to send a fresh one
/// rather than repeating the generic failure copy, so the two move together.
const LINK_EXPIRED: &str = "reset_link_expired";

/// `error` code for a link that was already spent, superseded by a newer
/// request, or minted against a password that has since changed.
const LINK_USED: &str = "reset_link_used";

/// `error` code for a disabled account, matching `POST /users/login`.
const ACCOUNT_DISABLED: &str = "account_disabled";

pub fn router() -> Router<AppState> {
    Router::new().route("/users/reset-password/confirm", post(confirm_reset))
}

async fn confirm_reset(
    State(state): State<AppState>,
    Json(req): Json<ConfirmResetRequest>,
) -> Result<Response, AppError> {
    let claims =
        verify_reset_token(&req.token, state.jwt_decoding_key()).map_err(|err| {
            match err.kind() {
                ErrorKind::ExpiredSignature => AppError::ForbiddenReason {
                    code: LINK_EXPIRED,
                    message: "That reset link has expired".into(),
                },
                _ => AppError::ForbiddenReason {
                    code: LINK_USED,
                    message: "That reset link isn't valid any more".into(),
                },
            }
        })?;

    let account = load_account(state.pool(), claims.sub)
        .await?
        .ok_or(link_used())?;

    // The link names one send. Anything else in the last slot means it was
    // already spent, a newer request replaced it, or the password moved on.
    let matches = account
        .password_reset_sends
        .as_deref()
        .and_then(<[DateTime<Utc>]>::last)
        .is_some_and(|last| last.timestamp_micros() == claims.stamp);
    if !matches {
        return Err(link_used());
    }

    let permissions = Permissions::from_bits_retain(account.permissions);
    if !permissions.contains(Permissions::CAN_AUTHORIZE) {
        return Err(AppError::ForbiddenReason {
            code: ACCOUNT_DISABLED,
            message: "This account is disabled".into(),
        });
    }

    let mut errors = FieldErrors::new();
    if let Some(message) = weak_password(&req.password) {
        errors.add("password", message);
    }
    errors.into_result()?;

    let body = apply(&state, claims.sub, &req.password).await?;
    let cookie = mint_session_cookie_at(
        claims.sub,
        body.me.user.name.clone(),
        permissions,
        body.signed_in_at,
        state.jwt_encoding_key(),
        state.https(),
    )
    .map_err(into_internal)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(into_internal)?,
    );
    Ok((StatusCode::OK, headers, Json(body.me)).into_response())
}

#[derive(Debug, Deserialize)]
struct ConfirmResetRequest {
    token: String,
    password: String,
}

#[derive(Debug, sqlx::FromRow)]
struct Account {
    permissions: i32,
    password_reset_sends: Option<Vec<DateTime<Utc>>>,
}

/// The refreshed user plus the instant the write stamped, which the new cookie
/// has to carry as its `iat`.
struct Applied {
    me: MeDto,
    signed_in_at: i64,
}

async fn load_account(pool: &PgPool, user_id: i32) -> Result<Option<Account>, AppError> {
    sqlx::query_as("SELECT permissions, password_reset_sends FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(into_internal)
}

async fn apply(state: &AppState, user_id: i32, new_password: &str) -> Result<Applied, AppError> {
    let hash = password::hash_argon2(new_password).map_err(|e| AppError::Internal(e.into()))?;
    // One instant for the stamp and the cookie's `iat`, so the slide middleware
    // doesn't read the cookie we're about to hand back as predating the stamp.
    let now = Utc::now();

    let mut q = Update::new("users");
    q.set("password_hash", hash);
    // Emptying the list spends the link, resets the ladder, and lets the user
    // ask for another one straight away.
    q.set("password_reset_sends", Vec::<DateTime<Utc>>::new());
    // Forcibly sign out any other existing session.
    q.set("sessions_valid_from", now);
    q.and_where("id = $", (user_id,));
    q.execute(state.pool()).await.map_err(into_internal)?;

    let me = fetch_me(state.pool(), user_id)
        .await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("user {user_id} vanished mid-reset")))?;

    Ok(Applied {
        me,
        signed_in_at: now.timestamp(),
    })
}

fn link_used() -> AppError {
    AppError::ForbiddenReason {
        code: LINK_USED,
        message: "That reset link isn't valid any more".into(),
    }
}
