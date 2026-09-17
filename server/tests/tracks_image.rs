//! HTTP integration tests for `GET /tracks/{id}/og.jpg`.

mod common;

use axum::{
    http::{StatusCode, header},
    response::Response,
};
use chrono::{DateTime, Utc};
use http_body_util::BodyExt;
use serde_json::json;
use serial_test::serial;
use sqlx::PgPool;
use tengri_formats::{Metadata, TengriFile, Track, TrackPoint, encode};
use tengri_scoring::{
    Route, RouteEvaluation, RoutePoint, RouteSubType, RouteType, RouteWaypoint, ScoringOutcome,
};
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
async fn the_picture_is_drawn_once_and_served_from_the_table() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let flight_id = common::seed_flight(&pool, "IMGCACHE", TEST_USER_ID).await;
    common::seed_full_track(&pool, &flight_id, sample_http_bytes()).await;
    let uri = format!("/tracks/{flight_id}/og.jpg");

    let first = app.clone().oneshot(common::get(&uri)).await.unwrap();
    let drawn_at = stored_at(&pool, &flight_id).await;
    let second = app.oneshot(common::get(&uri)).await.unwrap();

    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(body(first).await, body(second).await);
    assert_eq!(
        stored_at(&pool, &flight_id).await,
        drawn_at,
        "the second request should serve the stored row, not redraw it"
    );
}

#[tokio::test]
#[serial]
async fn a_matching_if_none_match_gets_a_304() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let flight_id = common::seed_flight(&pool, "IMG304", TEST_USER_ID).await;
    common::seed_full_track(&pool, &flight_id, sample_http_bytes()).await;
    let uri = format!("/tracks/{flight_id}/og.jpg");

    let first = app.clone().oneshot(common::get(&uri)).await.unwrap();
    let etag = first
        .headers()
        .get(header::ETAG)
        .expect("a stored picture carries an etag")
        .to_str()
        .unwrap()
        .to_owned();

    let revalidated = app
        .oneshot(common::get_if_none_match(&uri, &etag))
        .await
        .unwrap();

    assert_eq!(revalidated.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(revalidated.headers().get(header::ETAG).unwrap(), &etag);
    assert!(body(revalidated).await.is_empty());
}

#[tokio::test]
#[serial]
async fn scoring_a_flight_drops_its_picture() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let flight_id = common::seed_flight(&pool, "IMGSCORE", TEST_USER_ID).await;
    common::seed_full_track(&pool, &flight_id, sample_http_bytes()).await;

    app.oneshot(common::get(format!("/tracks/{flight_id}/og.jpg")))
        .await
        .unwrap();
    assert!(stored_at(&pool, &flight_id).await.is_some());

    store_scored_route(&pool, &flight_id).await;

    assert!(
        stored_at(&pool, &flight_id).await.is_none(),
        "the new route would be drawn differently, so the old picture has to go"
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

/// Write a scored route the way the scoring worker does — through
/// `upsert_scored_routes`, which is where the cached picture is invalidated.
async fn store_scored_route(pool: &PgPool, flight_id: &str) {
    let fix = |idx: usize, lat: i32, lon: i32| RouteWaypoint::Point {
        fix: RoutePoint { idx, lat, lon },
    };
    let evaluation = RouteEvaluation {
        routes: vec![ScoringOutcome::Answer(Route {
            id: 0,
            flight_id: flight_id.to_owned(),
            route_type: RouteType::FreeDistance,
            sub_type: RouteSubType::None,
            turnpoints: vec![fix(0, 4_677_248, 1_314_815), fix(119, 4_682_008, 1_325_525)],
            leg_distances: vec![10_000],
            distance: 10_000,
            score: 10.0,
            factor: 1.0,
            optimal: true,
            closure: None,
            scored_ms: 7,
        })],
    };

    let mut tx = pool.begin().await.expect("start the scoring transaction");
    tengri_server::flight::store::upsert_scored_routes(&mut tx, flight_id, &evaluation)
        .await
        .expect("store the scored route");
    tx.commit().await.expect("commit the scoring transaction");
}

/// When the flight's picture was last drawn, or `None` when there is none.
async fn stored_at(pool: &PgPool, flight_id: &str) -> Option<DateTime<Utc>> {
    sqlx::query_scalar("SELECT created_at FROM flight_images WHERE flight_id = $1")
        .bind(flight_id)
        .fetch_optional(pool)
        .await
        .expect("read the stored picture")
}

async fn body(response: Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

fn assert_jpeg(body: &[u8]) {
    assert_eq!(&body[..2], &[0xff, 0xd8], "SOI marker");
    assert_eq!(&body[body.len() - 2..], &[0xff, 0xd9], "EOI marker");
}
