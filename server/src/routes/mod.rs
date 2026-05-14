use axum::{Router, middleware::from_fn_with_state};

use crate::{AppState, auth::session_layer};

mod admin;
mod health;
mod me;
mod site;
mod tracks;
mod tracks_list;
mod tracks_md;
mod users;

/// `users::public_router()` (login/logout) bypasses the slide
/// middleware; everything else sits behind it. `site::public_router()`
/// also rides the session-aware tree — anonymous calls are allowed
/// but the handler doesn't extract `Identity`, so the slide layer
/// is a harmless no-op for them.
pub fn router(state: AppState) -> Router<AppState> {
    let session_aware = Router::new()
        .merge(admin::router())
        .merge(health::router())
        .merge(me::router())
        .merge(site::public_router())
        .merge(tracks::router())
        .merge(tracks_list::router())
        .merge(tracks_md::router())
        .merge(users::session_router())
        .layer(from_fn_with_state(state, session_layer));

    Router::new()
        .merge(users::public_router())
        .merge(session_aware)
}
