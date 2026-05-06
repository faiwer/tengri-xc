use axum::{Json, Router, extract::State, routing::post};
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/greet", post(greet))
}

#[derive(Deserialize)]
struct GreetRequest {
    name: String,
    excited: Option<bool>,
}

#[derive(Serialize)]
struct GreetResponse {
    message: String,
    request_id: u64,
}

async fn greet(
    State(state): State<AppState>,
    Json(payload): Json<GreetRequest>,
) -> Result<Json<GreetResponse>, AppError> {
    if payload.name.trim().is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }

    let punct = if payload.excited.unwrap_or(false) {
        "!"
    } else {
        "."
    };

    Ok(Json(GreetResponse {
        message: format!("Hello, {}{}", payload.name, punct),
        request_id: state.next_request_id(),
    }))
}
