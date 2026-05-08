//! HTTP integration tests for `GET /tracks` (cursor-paginated list).

mod common;

use axum::http::{StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use serial_test::serial;
use tower::ServiceExt;

const TEST_USER_ID: i32 = 1;
const TEST_USER_NAME: &str = "Test Pilot";

// Three flights with strictly decreasing takeoff times so the expected
// list order is unambiguous: NEWEST first, OLDEST last.
const TAKEOFF_NEWEST: i64 = 1_777_900_000;
const TAKEOFF_MIDDLE: i64 = 1_777_800_000;
const TAKEOFF_OLDEST: i64 = 1_777_700_000;

const FLIGHT_NEWEST: &str = "FLIGHT01";
const FLIGHT_MIDDLE: &str = "FLIGHT02";
const FLIGHT_OLDEST: &str = "FLIGHT03";

#[tokio::test]
#[serial]
async fn list_empty_db_returns_empty_page() {
    let (app, _pool) = common::test_app().await;

    let resp = app.oneshot(common::get("/tracks")).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json",
    );

    let json = body_as_json(resp).await;
    assert_eq!(json["items"].as_array().unwrap().len(), 0);
    assert!(json["next_cursor"].is_null());
}

#[tokio::test]
#[serial]
async fn list_returns_flights_newest_first_with_full_payload_shape() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    seed_three_flights(&pool).await;

    let resp = app.oneshot(common::get("/tracks")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = body_as_json(resp).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);

    // Order is newest-first.
    assert_eq!(items[0]["track"]["id"], FLIGHT_NEWEST);
    assert_eq!(items[1]["track"]["id"], FLIGHT_MIDDLE);
    assert_eq!(items[2]["track"]["id"], FLIGHT_OLDEST);

    // Payload shape on the first item.
    assert_eq!(items[0]["pilot"]["id"], TEST_USER_ID);
    assert_eq!(items[0]["pilot"]["name"], TEST_USER_NAME);
    assert_eq!(items[0]["track"]["takeoff_at"], TAKEOFF_NEWEST);
    assert_eq!(items[0]["track"]["duration"], 600); // landed = takeoff + 600

    // Final page → no next cursor.
    assert!(json["next_cursor"].is_null());
}

#[tokio::test]
#[serial]
async fn list_paginates_with_cursor() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    seed_three_flights(&pool).await;

    // Page 1: limit=2 → first two items + next_cursor.
    let resp = app
        .clone()
        .oneshot(common::get("/tracks?limit=2"))
        .await
        .unwrap();
    let json = body_as_json(resp).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["track"]["id"], FLIGHT_NEWEST);
    assert_eq!(items[1]["track"]["id"], FLIGHT_MIDDLE);
    let cursor = json["next_cursor"]
        .as_str()
        .expect("next_cursor present on full page")
        .to_owned();
    // Sanity: 12 raw bytes -> 16 base64url chars.
    assert_eq!(cursor.len(), 16);

    // Page 2: pass the cursor → last item only, no further cursor.
    let resp = app
        .oneshot(common::get(format!("/tracks?limit=2&cursor={cursor}")))
        .await
        .unwrap();
    let json = body_as_json(resp).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["track"]["id"], FLIGHT_OLDEST);
    assert!(json["next_cursor"].is_null());
}

#[tokio::test]
#[serial]
async fn list_breaks_takeoff_ties_by_id_and_paginates_across_them() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;

    // Two flights at the SAME takeoff_at second. Tie is broken by
    // `id DESC`, so "FLITTIE2" should land before "FLITTIE1".
    let same = 1_777_888_888;
    common::seed_flight_at(&pool, "FLITTIE1", TEST_USER_ID, same, same + 60).await;
    common::seed_flight_at(&pool, "FLITTIE2", TEST_USER_ID, same, same + 60).await;

    // Page 1 with limit=1 → must return FLITTIE2 first.
    let resp = app
        .clone()
        .oneshot(common::get("/tracks?limit=1"))
        .await
        .unwrap();
    let json = body_as_json(resp).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["track"]["id"], "FLITTIE2");
    let cursor = json["next_cursor"].as_str().unwrap().to_owned();

    // Page 2 with the cursor → must return FLITTIE1 (no skip, no dup).
    let resp = app
        .oneshot(common::get(format!("/tracks?limit=1&cursor={cursor}")))
        .await
        .unwrap();
    let json = body_as_json(resp).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["track"]["id"], "FLITTIE1");
    assert!(json["next_cursor"].is_null());
}

#[tokio::test]
#[serial]
async fn list_rejects_invalid_limit() {
    let (app, _pool) = common::test_app().await;

    let resp = app
        .clone()
        .oneshot(common::get("/tracks?limit=0"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = app.oneshot(common::get("/tracks?limit=101")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn list_rejects_malformed_cursor() {
    let (app, _pool) = common::test_app().await;

    // Bad base64.
    let resp = app
        .clone()
        .oneshot(common::get("/tracks?cursor=!!!not-base64!!!"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Well-formed base64, wrong decoded length (4 bytes -> 6 chars,
    // not the expected 12 -> 16).
    let resp = app
        .oneshot(common::get("/tracks?cursor=AAAAAA"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

async fn seed_three_flights(pool: &sqlx::PgPool) {
    common::seed_flight_at(
        pool,
        FLIGHT_NEWEST,
        TEST_USER_ID,
        TAKEOFF_NEWEST,
        TAKEOFF_NEWEST + 600,
    )
    .await;
    common::seed_flight_at(
        pool,
        FLIGHT_MIDDLE,
        TEST_USER_ID,
        TAKEOFF_MIDDLE,
        TAKEOFF_MIDDLE + 1200,
    )
    .await;
    common::seed_flight_at(
        pool,
        FLIGHT_OLDEST,
        TEST_USER_ID,
        TAKEOFF_OLDEST,
        TAKEOFF_OLDEST + 1800,
    )
    .await;
}

async fn body_as_json(resp: axum::response::Response) -> Value {
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).expect("response is valid JSON")
}
