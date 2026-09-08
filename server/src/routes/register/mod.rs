//! Self-service account creation and the confirmation click that finishes it.
//! Both routes are public: registration has no session yet, and the
//! confirmation link is opened from a mail client that carries no cookies.

use axum::Router;

use crate::{AppError, AppState};

mod confirm;
mod signup;

pub fn public_router() -> Router<AppState> {
    Router::new()
        .merge(signup::router())
        .merge(confirm::router())
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
