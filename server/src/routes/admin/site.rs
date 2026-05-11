//! `/admin/site` — operator settings editor. Both endpoints require the
//! `MANAGE_SETTINGS` bit. `GET` returns the full state including
//! raw markdown (the form populates its textareas from this);
//! `PATCH` accepts a partial update and returns the updated full
//! state so the FE can refresh its `useSite()` context in-place.

use axum::{Json, Router, extract::State, routing::get};

use crate::{
    AppError, AppState,
    auth::{Identity, require_permission},
    site::{
        AdminSiteDto, UpdateSiteRequest, apply_site_update, fetch_site_admin, validate_site_update,
    },
    user::Permissions,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/site", get(detail).patch(update))
}

async fn detail(
    State(state): State<AppState>,
    identity: Identity,
) -> Result<Json<AdminSiteDto>, AppError> {
    require_permission(&identity, Permissions::MANAGE_SETTINGS)?;
    fetch_site_admin(state.pool()).await.map(Json)
}

async fn update(
    State(state): State<AppState>,
    identity: Identity,
    Json(req): Json<UpdateSiteRequest>,
) -> Result<Json<AdminSiteDto>, AppError> {
    require_permission(&identity, Permissions::MANAGE_SETTINGS)?;

    let validated = validate_site_update(req).map_err(AppError::Validation)?;
    if validated.is_noop() {
        // Empty body / all-fields-absent: mirror `PATCH /users/me` and
        // reject as a misuse rather than silently re-fetching.
        return Err(AppError::BadRequest(
            "PATCH body must include at least one settable field".into(),
        ));
    }

    apply_site_update(state.pool(), &validated).await?;
    fetch_site_admin(state.pool()).await.map(Json)
}
