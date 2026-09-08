//! `/users/*` — auth and current-user.
//!
//! - `POST /users/login`  — `{ identifier, password }` → cookie + `/users/me`
//!   body. Identifier matches `login` or `email`, case-insensitively. 403 when
//!   the account holds an unconfirmed address; see `routes::register`.
//! - `POST /users/logout` — clear the cookie. Always 204.
//! - `GET  /users/me`     — current user, or `null` if anonymous. Always 200.
//! - `PATCH /users/me`    — owner-self profile/preferences edit. A changed
//!   email goes to `pending_email` and gets a confirmation link mailed to it
//!   rather than being written straight through; see
//!   [`crate::user::plan_email_edit`].
//! - `POST /users/me/password` — owner-self change/set password. Sets an
//!   initial `login` too when the account has none. Returns the refreshed
//!   `/users/me` body.

use axum::{
    Router,
    routing::{get, post},
};

use crate::{AppError, AppState};

mod login;
mod logout;
mod me;
mod password;
mod update_me;

/// Routes that set/clear the cookie inline; mounted *outside*
/// the slide middleware.
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/users/login", post(login::login))
        .route("/users/logout", post(logout::logout))
}

/// Routes that read identity from extensions; mounted behind the
/// slide middleware.
pub fn session_router() -> Router<AppState> {
    Router::new()
        .route("/users/me", get(me::me).patch(update_me::update_me))
        .route("/users/me/password", post(password::change_password))
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
