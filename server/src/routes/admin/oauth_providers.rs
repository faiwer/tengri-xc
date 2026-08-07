//! `/admin/oauth-providers` — per-provider OAuth credential editor. Both
//! endpoints require the `MANAGE_SETTINGS` bit, same as `/admin/site`.
//!
//! `GET` returns only the configured providers (including their secrets, since
//! the editor prefills from them); the client merges this against its own
//! canonical provider list. `PATCH /:provider` accepts a partial update and
//! returns the refreshed full list so the FE can re-render in place.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, patch},
};

use crate::{
    AppError, AppState,
    auth::{Identity, require_permission},
    oauth::{
        AdminOAuthProviderDto, OAuthProvider, UpdateOAuthProviderRequest,
        apply_oauth_provider_update, fetch_oauth_providers_admin, validate_oauth_provider_update,
    },
    user::Permissions,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/oauth-providers", get(list))
        .route("/admin/oauth-providers/{provider}", patch(update))
}

async fn list(
    State(state): State<AppState>,
    identity: Identity,
) -> Result<Json<Vec<AdminOAuthProviderDto>>, AppError> {
    require_permission(&identity, Permissions::MANAGE_SETTINGS)?;
    fetch_oauth_providers_admin(state.pool()).await.map(Json)
}

async fn update(
    State(state): State<AppState>,
    identity: Identity,
    Path(provider): Path<OAuthProvider>,
    Json(req): Json<UpdateOAuthProviderRequest>,
) -> Result<Json<Vec<AdminOAuthProviderDto>>, AppError> {
    require_permission(&identity, Permissions::MANAGE_SETTINGS)?;

    let update = validate_oauth_provider_update(state.pool(), provider, req).await?;
    apply_oauth_provider_update(state.pool(), provider, &update).await?;
    fetch_oauth_providers_admin(state.pool()).await.map(Json)
}
