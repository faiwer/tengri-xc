use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

/// Top-level error type for handlers.
///
/// Add new variants as the surface grows. Keep `Internal` as the catch-all for
/// unexpected failures; map known/expected failures to specific variants so the
/// API contract stays explicit.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),

    #[error("not found")]
    NotFound,

    #[error("unauthorized")]
    Unauthorized,

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    message: String,
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            AppError::BadRequest(_) => "bad_request",
            AppError::NotFound => "not_found",
            AppError::Unauthorized => "unauthorized",
            AppError::Internal(_) => "internal_error",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Internal errors get logged at error level; the response body stays
        // generic so we don't leak implementation details to callers.
        if let AppError::Internal(ref err) = self {
            tracing::error!(error = %err, "internal server error");
        }

        let status = self.status();
        let code = self.code();
        let message = match &self {
            AppError::Internal(_) => "internal server error".to_owned(),
            other => other.to_string(),
        };

        (
            status,
            Json(ErrorBody {
                error: code,
                message,
            }),
        )
            .into_response()
    }
}
