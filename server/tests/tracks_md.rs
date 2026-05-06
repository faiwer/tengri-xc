//! HTTP integration tests for `GET /tracks/{id}/md`.

mod common;

use axum::http::{StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use serial_test::serial;
use tower::ServiceExt;

const TEST_USER_ID: i32 = 1;
const TEST_USER_NAME: &str = "Test Pilot";

#[tokio::test]
#[serial]
async fn track_md_returns_id_and_pilot_name() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let flight_id = common::seed_flight(&pool, "MDTEST00", TEST_USER_ID).await;

    let resp = app
        .oneshot(common::get(format!("/tracks/{flight_id}/md")))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json",
    );

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).expect("response is valid JSON");

    assert_eq!(json["id"], flight_id.as_str());
    assert_eq!(json["pilot"]["name"], TEST_USER_NAME);
}

#[tokio::test]
#[serial]
async fn track_md_unknown_id_returns_404() {
    let (app, _pool) = common::test_app().await;

    let resp = app
        .oneshot(common::get("/tracks/NOPE0000/md"))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
