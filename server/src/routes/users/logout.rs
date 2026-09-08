use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};

use super::into_internal;
use crate::{AppError, AppState, auth::cookie::clear_session};

/// 204 even if there was no session — idempotent, and avoids
/// noisy 401s when the client logs out twice.
pub(super) async fn logout(State(state): State<AppState>) -> Result<Response, AppError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&clear_session(state.https())).map_err(into_internal)?,
    );
    Ok((StatusCode::NO_CONTENT, headers).into_response())
}
