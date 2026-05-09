use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use sqlx::postgres::PgPoolOptions;
use tengri_server::{AppState, build_app};
use tower::ServiceExt;

#[tokio::test]
async fn unknown_route_returns_404() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://test:test@localhost/test")
        .expect("build lazy pool");
    let app = build_app(AppState::new(pool, &[0u8; 32], false));

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
