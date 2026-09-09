//! Forgotten-password recovery: mail a link to a proven address, then spend it
//! on a new password. Both routes are public — the whole point is that the
//! caller can't sign in.

use axum::Router;

use crate::{AppError, AppState};

mod confirm;
mod ladder;
mod request;

pub fn public_router() -> Router<AppState> {
    Router::new()
        .merge(request::router())
        .merge(confirm::router())
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
