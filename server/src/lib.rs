pub mod auth;
pub mod config;
pub mod error;
pub mod flight;
pub mod geo;
pub mod routes;
pub mod state;
pub mod telemetry;
pub mod user;

use axum::Router;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

pub use crate::{config::Config, error::AppError, state::AppState};

pub fn build_app(state: AppState) -> Router {
    // CORS: permissive for now. Tighten by replacing `Any` with explicit origins
    // once the client deployment story is settled.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

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
