use axum::{Json, extract::State};

use crate::{
    AppError, AppState,
    auth::Identity,
    user::{MeDto, fetch_me},
};

/// `null` for anonymous, current user otherwise. Always 200 — "nobody" is a
/// valid answer here, and 401 would just spam the browser console with red
/// errors on every anon SPA boot.
pub(super) async fn me(
    State(state): State<AppState>,
    identity: Option<Identity>,
) -> Result<Json<Option<MeDto>>, AppError> {
    let Some(identity) = identity else {
        return Ok(Json(None));
    };
    // Row missing = user hard-deleted between the last slide and now. Treat as
    // anonymous; the next request slides cleanly.
    Ok(Json(fetch_me(state.pool(), identity.user_id).await?))
}
