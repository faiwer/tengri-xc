//! `GET /users/confirm-email?token=` — prove the address a token was minted
//! for, then bounce the browser back to the SPA. Opened from a mail client, so
//! it always redirects and never renders, and reports what happened through a
//! query param the SPA reads off the landing URL. Serves both registration and
//! the address change from `PATCH /users/me`.

use axum::{
    Router,
    extract::{Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{LOCATION, SET_COOKIE},
    },
    response::{IntoResponse, Response},
    routing::get,
};
use jsonwebtoken::errors::ErrorKind;
use serde::Deserialize;
use sqlx::PgPool;

use super::into_internal;
use crate::{
    AppError, AppState,
    auth::{token::mint_session_cookie, verify_confirm_token},
    user::Permissions,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/users/confirm-email", get(confirm_email))
}

async fn confirm_email(
    State(state): State<AppState>,
    Query(query): Query<ConfirmQuery>,
) -> Result<Response, AppError> {
    let claims = match verify_confirm_token(&query.token, state.jwt_decoding_key()) {
        Ok(claims) => claims,
        Err(err) => {
            let reason = match err.kind() {
                ErrorKind::ExpiredSignature => "expired",
                _ => "invalid",
            };
            return redirect(&state, ERR_PARAM, reason, None);
        }
    };

    let Some(account) = load_account(state.pool(), claims.sub).await? else {
        return redirect(&state, ERR_PARAM, "invalid", None);
    };

    let outcome = prove_address(state.pool(), claims.sub, &account, &claims.email).await?;
    let (param, value) = outcome.landing();

    // Confirming a banned account's address is harmless; handing it a session
    // is not.
    let permissions = Permissions::from_bits_retain(account.permissions);
    if !outcome.earns_a_session() || !permissions.contains(Permissions::CAN_AUTHORIZE) {
        return redirect(&state, param, value, None);
    }

    let cookie = mint_session_cookie(
        claims.sub,
        account.name,
        permissions,
        state.jwt_encoding_key(),
        state.https(),
    )
    .map_err(into_internal)?;
    redirect(&state, param, value, Some(&cookie))
}

#[derive(Debug, Deserialize)]
struct ConfirmQuery {
    #[serde(default)]
    token: String,
}

/// What the click did to the account's two address columns.
enum Outcome {
    /// The account's first proven address — a registration finishing.
    Confirmed,
    /// A pending address replaced the proven one.
    Changed,
    /// Another account proved the same address first.
    Taken,
    /// The token names an address this account has since moved off.
    Stale,
}

impl Outcome {
    /// `(param, value)` for the SPA landing URL.
    fn landing(&self) -> (&'static str, &'static str) {
        match self {
            Self::Confirmed => (OK_PARAM, "confirmed"),
            Self::Changed => (OK_PARAM, "changed"),
            Self::Taken => (ERR_PARAM, "taken"),
            Self::Stale => (ERR_PARAM, "invalid"),
        }
    }

    fn earns_a_session(&self) -> bool {
        matches!(self, Self::Confirmed | Self::Changed)
    }
}

#[derive(sqlx::FromRow)]
struct Account {
    name: String,
    email: Option<String>,
    pending_email: Option<String>,
    permissions: i32,
}

async fn load_account(pool: &PgPool, user_id: i32) -> Result<Option<Account>, AppError> {
    sqlx::query_as("SELECT name, email, pending_email, permissions FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(into_internal)
}

/// Match the token's address against the account's columns and write whatever
/// the match implies.
async fn prove_address(
    pool: &PgPool,
    user_id: i32,
    account: &Account,
    claimed: &str,
) -> Result<Outcome, AppError> {
    if account.pending_email.as_deref() == Some(claimed) {
        // A pending address being proven — a registration finishing if there's
        // no address yet, a change finishing if there is. The unique index is
        // the arbiter either way: someone else may have proven the same address
        // between this link being minted and clicked.
        let replaced_an_address = account.email.is_some();
        return match promote(pool, user_id).await {
            Ok(()) if replaced_an_address => Ok(Outcome::Changed),
            Ok(()) => Ok(Outcome::Confirmed),
            Err(err) if is_email_conflict(&err) => Ok(Outcome::Taken),
            Err(err) => Err(into_internal(err)),
        };
    }

    if account.email.as_deref() == Some(claimed) {
        // A second click on a link that already promoted — a prefetching mail
        // scanner, a back button, a forwarded message. The promote stamped the
        // address, so there's nothing left to write; say "confirmed" instead of
        // sending someone whose address *is* confirmed to an error page.
        return Ok(Outcome::Confirmed);
    }

    Ok(Outcome::Stale)
}

/// Move `pending_email` into `email` and stamp it proven.
async fn promote(pool: &PgPool, user_id: i32) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE users \
         SET email = pending_email, pending_email = NULL, email_verified_at = now() \
         WHERE id = $1",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Whether `err` is the `users_email_key` unique violation, i.e. the address
/// was proven by another account first.
fn is_email_conflict(err: &sqlx::Error) -> bool {
    err.as_database_error().and_then(|db| db.constraint()) == Some("users_email_key")
}

/// 303 to the SPA root with `?{param}={value}`, optionally setting `cookie`.
fn redirect(
    state: &AppState,
    param: &str,
    value: &str,
    cookie: Option<&str>,
) -> Result<Response, AppError> {
    let url = format!("{}/?{param}={value}", state.app_base_url());
    let location = HeaderValue::from_str(&url)
        .map_err(|_| into_internal(anyhow::anyhow!("APP_BASE_URL isn't header-safe: {url:?}")))?;

    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, location);
    if let Some(cookie) = cookie {
        let cookie = HeaderValue::from_str(cookie)
            .map_err(|_| into_internal(anyhow::anyhow!("session cookie isn't header-safe")))?;
        headers.insert(SET_COOKIE, cookie);
    }
    Ok((StatusCode::SEE_OTHER, headers).into_response())
}

/// Query params the SPA's `EmailConfirmHandler` reads off the landing URL.
const OK_PARAM: &str = "email";
const ERR_PARAM: &str = "email_error";
