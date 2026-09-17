//! HTTP integration tests for `GET /tracks/{id}/og.jpg`.

mod common;

use axum::http::{StatusCode, header};
use http_body_util::BodyExt;
use serde_json::json;
use serial_test::serial;
use sqlx::PgPool;
use tengri_formats::{Metadata, TengriFile, Track, TrackPoint, encode};
use tower::ServiceExt;

const TEST_USER_ID: i32 = 1;
const TEST_USER_NAME: &str = "Test Pilot";

/// A short climb-out, encoded through the production path so the handler
/// decodes real bytes rather than a hand-rolled blob.
fn sample_http_bytes() -> Vec<u8> {
    let track = Track {
        start_time: 1_700_000_000,
        points: (0..120)
            .map(|i| TrackPoint {
                time: 1_700_000_000 + i as u32,
                lat: 4_677_248 + i * 40,
                lon: 1_314_815 + i * 90,
                geo_alt: 17_350 + i,
                pressure_alt: Some(16_640 + i),
                tas: None,
            })
            .collect(),
    };
    let envelope = TengriFile::new(Metadata::default(), encode(&track).unwrap());
    envelope.to_http_bytes().unwrap()
}

#[tokio::test]
#[serial]
async fn a_scored_flight_draws_its_waypoints_over_the_track() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let flight_id = common::seed_flight(&pool, "IMGROUTE", TEST_USER_ID).await;
    common::seed_full_track(&pool, &flight_id, sample_http_bytes()).await;
    seed_main_route(&pool, &flight_id).await;

    let resp = app
        .oneshot(common::get(format!("/tracks/{flight_id}/og.jpg")))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/jpeg"
    );
    assert_jpeg(
        resp.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .as_ref(),
    );
}

#[tokio::test]
#[serial]
async fn an_unscored_flight_draws_the_track_alone() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let flight_id = common::seed_flight(&pool, "IMGTRACK", TEST_USER_ID).await;
    common::seed_full_track(&pool, &flight_id, sample_http_bytes()).await;

    let resp = app
        .oneshot(common::get(format!("/tracks/{flight_id}/og.jpg")))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_jpeg(
        resp.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .as_ref(),
    );
}

#[tokio::test]
#[serial]
async fn track_image_unknown_id_returns_404() {
    let (app, _pool) = common::test_app().await;

    let resp = app
        .oneshot(common::get("/tracks/NOPE0000/og.jpg"))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// An OLC free triangle, stored the way the scoring worker leaves it: three
/// turnpoints, a closure, and the flight pointing at the row.
async fn seed_main_route(pool: &PgPool, flight_id: &str) {
    let turnpoint = |lat: i32, lon: i32| json!({ "type": "point", "fix": { "idx": 0, "lat": lat, "lon": lon } });
    let route_id: i64 = sqlx::query_scalar(
        "INSERT INTO routes \
            (flight_id, type, sub_type, turnpoints, leg_distances, distance, score, factor, \
             optimal, closure, scored_ms) \
         VALUES ($1, 'free_triangle', 'olc_open', $2::jsonb, ARRAY[10000, 10000, 10000], 30000, \
                 42.00, 1.4, true, $3::jsonb, 12) \
         RETURNING id",
    )
    .bind(flight_id)
    .bind(
        json!([
            turnpoint(4_250_000, 7_400_000),
            turnpoint(4_270_000, 7_430_000),
            turnpoint(4_240_000, 7_450_000),
        ])
        .to_string(),
    )
    .bind(
        json!({
            "start": turnpoint(4_200_000, 7_400_000),
            "end": turnpoint(4_210_000, 7_410_000),
            "distance": 5000,
        })
        .to_string(),
    )
    .fetch_one(pool)
    .await
    .expect("seed route");

    sqlx::query(
        "UPDATE flights \
         SET main_route_id = $2, main_route_type = 'free_triangle', main_score = 42.00, \
             main_distance = 30000 \
         WHERE id = $1",
    )
    .bind(flight_id)
    .bind(route_id)
    .execute(pool)
    .await
    .expect("point the flight at its main route");
}

fn assert_jpeg(body: &[u8]) {
    assert_eq!(&body[..2], &[0xff, 0xd8], "SOI marker");
    assert_eq!(&body[body.len() - 2..], &[0xff, 0xd9], "EOI marker");
}
