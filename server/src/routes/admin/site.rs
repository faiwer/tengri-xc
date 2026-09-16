//! `/admin/site` — operator settings editor. Every endpoint here requires the
//! `MANAGE_SETTINGS` bit. `GET` returns the full state including
//! raw markdown (the form populates its textareas from this);
//! `PATCH` accepts a partial update and returns the updated full
//! state so the FE can refresh its `useSite()` context in-place.

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};

use crate::{
    AppError, AppState,
    auth::{Identity, require_permission},
    mail::{TestEmailRequest, send_test_email},
    site::{
        AdminSiteDto, UpdateSiteRequest, apply_site_update, fetch_site_admin, validate_site_update,
    },
    user::Permissions,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/site", get(detail).patch(update))
        .route("/admin/site/test-email", post(test_email))
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
    // The name/description ride in every server-rendered <head>; drop the
    // cached copy so the next document request picks the edit up.
    state.clear_site_meta();
    fetch_site_admin(state.pool()).await.map(Json)
}

/// Send a one-off message to prove the relay works. The body carries the
/// editor's unsaved settings, so this can be used before `PATCH` commits them;
/// the stored row contributes only the fallback password and the site name.
async fn test_email(
    State(state): State<AppState>,
    identity: Identity,
    Json(req): Json<TestEmailRequest>,
) -> Result<StatusCode, AppError> {
    require_permission(&identity, Permissions::MANAGE_SETTINGS)?;

    let stored = fetch_site_admin(state.pool()).await?;
    send_test_email(req, &stored).await?;
    Ok(StatusCode::NO_CONTENT)
}
