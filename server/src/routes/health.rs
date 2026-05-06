use axum::{Json, Router, routing::get};
use serde::Serialize;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}
