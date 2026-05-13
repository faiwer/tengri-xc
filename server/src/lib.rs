pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod flight;
pub mod geo;
pub mod glider;
pub mod routes;
pub mod site;
pub mod state;
pub mod telemetry;
pub mod user;
pub mod validation;

use axum::{Router, http::HeaderValue};
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

pub use crate::{config::Config, error::AppError, state::AppState};

pub fn build_app(state: AppState) -> Router {
    // The session cookie is `SameSite=Lax`, so cross-origin XHR
    // only carries it with `credentials: 'include'` *and*
    // `Access-Control-Allow-Credentials: true`. That in turn
    // forbids the wildcard origin — list real origins via
    // `CLIENT_ORIGINS` (comma-separated). Empty list = same-origin
    // only, which is fine when the SPA is served by us.
    let mut cors = CorsLayer::new()
        .allow_credentials(true)
        .allow_methods(tower_http::cors::AllowMethods::mirror_request())
        .allow_headers(tower_http::cors::AllowHeaders::mirror_request());
    for origin in state.client_origins() {
        match HeaderValue::from_str(origin) {
            Ok(v) => cors = cors.allow_origin(v),
            Err(e) => tracing::warn!(%origin, error = %e, "ignoring invalid CLIENT_ORIGINS entry"),
        }
    }

    let trace = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(
            DefaultOnResponse::new()
                .level(Level::INFO)
                .latency_unit(tower_http::LatencyUnit::Millis),
        );

    routes::router(state.clone())
        .with_state(state)
        .layer(cors)
        .layer(trace)
}
