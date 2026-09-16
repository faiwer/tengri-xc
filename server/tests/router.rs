use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use sqlx::postgres::PgPoolOptions;
use tengri_server::{AppState, build_app, build_root};
use tower::ServiceExt;

mod common;
use common::PLACEHOLDER_DB_URL;

#[tokio::test]
async fn unknown_route_returns_404() {
    let pool = PgPoolOptions::new()
        .connect_lazy(PLACEHOLDER_DB_URL)
        .expect("build lazy pool");
    let app = build_app(AppState::new_for_tests(pool, &[0u8; 32], false));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// The SPA fallback answers unknown paths with `index.html`, but a path that
/// names a file belongs to the static host. The guard fires before the fetch,
/// so this needs neither a reachable `app_base_url` nor a live database.
#[tokio::test]
async fn static_asset_path_bypasses_the_spa_fallback() {
    let pool = PgPoolOptions::new()
        .connect_lazy(PLACEHOLDER_DB_URL)
        .expect("build lazy pool");
    let app = build_root(AppState::new_for_tests(pool, &[0u8; 32], false));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
