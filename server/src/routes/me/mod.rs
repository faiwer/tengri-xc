//! `/me/*` — owner-self routes for the signed-in pilot. These sit beside
//! `/users/me` (which lives in [`crate::routes::users`] for historical layout
//! reasons) and gate on a session, not on a permission bit. The split between
//! `/users/me` and `/me/*` is a convention: the former is "my
//! profile/preferences", the latter "my data" (gliders, eventually
//! tracks-as-owner, etc.).

use axum::Router;

use crate::AppState;

pub mod flights;
pub mod glider_catalog;
pub mod gliders;
pub mod recent_gliders;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(flights::router())
        .merge(gliders::router())
        .merge(glider_catalog::router())
        .merge(recent_gliders::router())
}
