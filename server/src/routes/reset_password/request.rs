//! `POST /users/reset-password` — mail a reset link to a proven address.
//!
//! Always 204, whether or not the address belongs to an account, so the form
//! can't be used to find out who has one here.

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;

use super::{
    into_internal,
    ladder::{next_allowed, push_send, recent_sends},
};
use crate::{
    AppError, AppState,
    mail::{ResetRecipient, is_smtp_configured, send_password_reset_email},
    site::fetch_site_admin,
    user::{Permissions, find_user_id_by_email, validate_email},
    validation::FieldErrors,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/users/reset-password", post(request_reset))
}

async fn request_reset(
    State(state): State<AppState>,
    Json(input): Json<ResetRequest>,
) -> Result<StatusCode, AppError> {
    let email = validate(input)?;

    // Refusing here says nothing about the address — it's the same answer for
    // every caller — so it doesn't undo the silence below.
    let stored = fetch_site_admin(state.pool()).await?;
    if !is_smtp_configured(&stored) {
        return Err(AppError::Conflict(
            "Password reset is unavailable right now — outgoing mail isn't configured".into(),
        ));
    }

    let Some(user_id) = find_user_id_by_email(state.pool(), &email).await? else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let Some(account) = load_account(state.pool(), user_id).await? else {
        return Ok(StatusCode::NO_CONTENT);
    };
    // A disabled account has nothing to come back to, so mailing it a link
    // would only be noise. Silent, not an error: saying so would leak that the
    // address is registered.
    if !Permissions::from_bits_retain(account.permissions).contains(Permissions::CAN_AUTHORIZE) {
        return Ok(StatusCode::NO_CONTENT);
    }

    let now = Utc::now();
    let recent = recent_sends(account.password_reset_sends.as_deref().unwrap_or(&[]), now);
    if next_allowed(&recent).is_some_and(|at| at > now) {
        // Too early to try again.
        return Ok(StatusCode::NO_CONTENT);
    }

    let mut tx = state.pool().begin().await.map_err(into_internal)?;
    let claimed = claim_send(
        &mut tx,
        user_id,
        account.password_reset_sends.as_deref(),
        &push_send(recent, now),
    )
    .await?;
    // Someone else's concurrent request got there first, so a link is already
    // on its way to the same inbox.
    if !claimed {
        return Ok(StatusCode::NO_CONTENT);
    }

    // Sent inside the transaction so a relay that rejects the message rolls the
    // ladder back rather than charging the user a wait for a mail they never
    // got. Costs holding the connection for the SMTP timeout.
    send_password_reset_email(
        &stored,
        state.app_base_url(),
        state.jwt_encoding_key(),
        ResetRecipient {
            user_id,
            name: &account.name,
            email: &email,
            stamp: now.timestamp_micros(),
        },
    )
    .await?;

    tx.commit().await.map_err(into_internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct ResetRequest {
    email: String,
}

#[derive(Debug, sqlx::FromRow)]
struct Account {
    name: String,
    permissions: i32,
    password_reset_sends: Option<Vec<DateTime<Utc>>>,
}

/// A malformed address is the caller's own typo, not a fact about anyone's
/// account, so rejecting it leaks nothing.
fn validate(input: ResetRequest) -> Result<String, AppError> {
    let mut errors = FieldErrors::new();
    if input.email.trim().is_empty() {
        errors.add("email", "Cannot be empty");
    }
    let email = validate_email(Some(input.email), &mut errors);
    errors.into_result()?;

    email.ok_or_else(|| AppError::Internal(anyhow::anyhow!("email validated but produced none")))
}

async fn load_account(pool: &PgPool, user_id: i32) -> Result<Option<Account>, AppError> {
    sqlx::query_as("SELECT name, permissions, password_reset_sends FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(into_internal)
}

/// Compare-and-swap on the send list: a burst of concurrent requests all read
/// the same `expected`, and only the one that commits first matches it. The
/// rest update nothing and are treated as throttled.
async fn claim_send(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i32,
    expected: Option<&[DateTime<Utc>]>,
    next: &[DateTime<Utc>],
) -> Result<bool, AppError> {
    let updated = sqlx::query(
        "UPDATE users \
         SET password_reset_sends = $2 \
         WHERE id = $1 AND password_reset_sends IS NOT DISTINCT FROM $3",
    )
    .bind(user_id)
    .bind(next)
    .bind(expected)
    .execute(&mut **tx)
    .await
    .map_err(into_internal)?;

    Ok(updated.rows_affected() == 1)
}
