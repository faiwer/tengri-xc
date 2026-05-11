//! HTTP integration tests for `GET /tracks/{id}/md`.

mod common;

use axum::http::{StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use serial_test::serial;
use tower::ServiceExt;

const TEST_USER_ID: i32 = 1;
const TEST_USER_NAME: &str = "Test Pilot";

// Arbitrary fixed instants (2026-05-03 UTC). Picked so the assertion is
// deterministic and doesn't drift with `now()`.
const TAKEOFF_AT: i64 = 1_777_887_122; // 2026-05-03T10:52:02Z
const LANDING_AT: i64 = 1_777_896_062; // 2026-05-03T13:21:02Z

#[tokio::test]
#[serial]
async fn track_md_returns_id_and_pilot_name() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let flight_id =
        common::seed_flight_at(&pool, "MDTEST00", TEST_USER_ID, TAKEOFF_AT, LANDING_AT).await;
    // The /md endpoint joins flight_tracks for compression_ratio; without
    // a matching row the JOIN drops the flight and we'd 404 here.
    common::seed_full_track(&pool, &flight_id, vec![0; 4]).await;

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
    assert_eq!(json["takeoff_at"], TAKEOFF_AT);
    assert_eq!(json["landing_at"], LANDING_AT);
    // Offsets and points come from the seed defaults (zeros + `(0, 0)`), so the
    // assertion is on shape and values: the route projects them through
    // `EXTRACT` / `ST_Y` / `ST_X` and the JSON stays numeric.
    assert_eq!(json["takeoff_offset"], 0);
    assert_eq!(json["landing_offset"], 0);
    assert_eq!(json["takeoff"]["lat"], 0.0);
    assert_eq!(json["takeoff"]["lon"], 0.0);
    assert_eq!(json["landing"]["lat"], 0.0);
    assert_eq!(json["landing"]["lon"], 0.0);
    // Sentinel `1.0` from `seed_full_track`. We just verify the field is
    // present and a number; the precise value isn't part of the API
    // contract here.
    assert!(json["compression_ratio"].is_number());
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
