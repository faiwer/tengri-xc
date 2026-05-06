use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tengri_server::{AppState, build_app};
use tower::ServiceExt;

#[tokio::test]
async fn unknown_route_returns_404() {
    let app = build_app(AppState::new());

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
