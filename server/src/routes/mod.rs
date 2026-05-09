use axum::{Router, middleware::from_fn_with_state};

use crate::{AppState, auth::session_layer};

mod admin;
mod health;
mod tracks;
mod tracks_list;
mod tracks_md;
mod users;

/// `users::public_router()` (login/logout) bypasses the slide
/// middleware; everything else sits behind it.
pub fn router(state: AppState) -> Router<AppState> {
    let session_aware = Router::new()
        .merge(admin::router())
        .merge(health::router())
        .merge(tracks::router())
        .merge(tracks_list::router())
        .merge(tracks_md::router())
        .merge(users::session_router())
        .layer(from_fn_with_state(state, session_layer));

    Router::new()
        .merge(users::public_router())
        .merge(session_aware)
}
