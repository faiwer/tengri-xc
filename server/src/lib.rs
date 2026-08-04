pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod flight;
pub mod glider;
pub mod ids;
pub mod migrate;
pub mod routes;
pub mod site;
pub mod state;
pub mod telemetry;
pub mod user;
pub mod validation;

use axum::{
    Router,
    http::{Extensions, HeaderMap, HeaderValue, StatusCode, Version, header},
};
use tower_http::{
    compression::{
        CompressionLayer,
        predicate::{DefaultPredicate, Predicate},
    },
    cors::CorsLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

pub use crate::{config::Config, error::AppError, state::AppState};

pub fn build_app(state: AppState) -> Router {
    let trace = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(
            DefaultOnResponse::new()
                .level(Level::INFO)
                .latency_unit(tower_http::LatencyUnit::Millis),
        );

    routes::router(state.clone())
        .with_state(state)
        .layer(trace)
        .layer(CompressionLayer::new().compress_when(DefaultPredicate::new().and(is_json_response)))
}

/// CORS for the SPA. The `SameSite=Lax` session cookie only rides cross-origin
/// XHR with `credentials: 'include'` *and* `Access-Control-Allow-Credentials:
/// true`, which forbids the wildcard origin — so echo only the configured
/// `origins` (`CLIENT_ORIGINS`). Empty list = same-origin only, fine when the
/// SPA is served by us.
///
/// Applied as the outermost layer (see `main`) so it covers the 404 fallback
/// too. Layered inside the router it would miss unmatched routes, and a
/// headerless 404 reads to the browser as a CORS error rather than a plain 404.
pub fn cors_layer(origins: &[String]) -> CorsLayer {
    let mut cors = CorsLayer::new()
        .allow_credentials(true)
        .allow_methods(tower_http::cors::AllowMethods::mirror_request())
        .allow_headers(tower_http::cors::AllowHeaders::mirror_request());
    for origin in origins {
        match HeaderValue::from_str(origin) {
            Ok(v) => cors = cors.allow_origin(v),
            Err(e) => tracing::warn!(%origin, error = %e, "ignoring invalid CLIENT_ORIGINS entry"),
        }
    }
    cors
}

fn is_json_response(
    _status: StatusCode,
    _version: Version,
    headers: &HeaderMap,
    _extensions: &Extensions,
) -> bool {
    let Some(content_type) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };

    let media_type = content_type
        .split_once(';')
        .map_or(content_type, |(media_type, _)| media_type)
        .trim();

    media_type.eq_ignore_ascii_case("application/json")
}
