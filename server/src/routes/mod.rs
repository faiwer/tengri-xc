use axum::Router;

use crate::AppState;

mod greet;
mod health;

/// Top-level router. Mount sub-routers here as the API grows; group related
/// handlers into their own modules and expose a `pub fn router() -> Router<AppState>`
/// from each, then `.merge` or `.nest` them in below.
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .nest("/api", api_router())
}

fn api_router() -> Router<AppState> {
    Router::new().merge(greet::router())
}
