//! Admin-only HTTP routes. Each handler gates on a `Permissions` bit via
//! [`crate::auth::require_permission`] — there is no blanket "admin" check,
//! because the bits aren't a ladder. A user with only `MANAGE_TRACKS` shouldn't
//! be able to list users, etc.

use axum::Router;

use crate::AppState;

pub mod gliders;
pub mod site;
pub mod users;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(users::router())
        .merge(site::router())
        .merge(gliders::router())
}
