//! HTTP integration tests for `GET /tracks/full/:id`.
//!
//! Backed by a dedicated `tengri_test` database — see `tests/common/mod.rs`.
//! Each test seeds exactly the rows it needs via the `common::seed_*`
//! helpers; no external bootstrap step required.

mod common;

use axum::http::{StatusCode, header};
use http_body_util::BodyExt;
use serial_test::serial;
use tengri_server::flight::{Metadata, TengriFile, Track, TrackPoint, encode};
use tower::ServiceExt;

const TEST_USER_ID: i32 = 1;
const TEST_USER_NAME: &str = "Test Pilot";

/// Build a small but realistic HTTP-form blob via the same code path
/// production uses, so tests exercise the actual encoder rather than fake
/// bytes that happen to look gzip-shaped.
fn sample_http_bytes() -> Vec<u8> {
    let track = Track {
        start_time: 1_700_000_000,
        points: vec![
            TrackPoint {
                time: 1_700_000_000,
                lat: 4_677_248,
                lon: 1_314_815,
                geo_alt: 17_350,
                pressure_alt: Some(16_640),
            },
            TrackPoint {
                time: 1_700_000_001,
                lat: 4_677_251,
                lon: 1_314_817,
                geo_alt: 17_355,
                pressure_alt: Some(16_645),
            },
        ],
    };
    let envelope = TengriFile::new(Metadata::default(), encode(&track).unwrap());
    envelope.to_http_bytes().unwrap()
}

#[tokio::test]
#[serial]
async fn full_track_serves_blob_with_etag_and_gzip_encoding() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let flight_id = common::seed_flight(&pool, "BLOBTEST", TEST_USER_ID).await;
    let bytes = sample_http_bytes();
    let etag = common::seed_full_track(&pool, &flight_id, bytes.clone()).await;

    let resp = app
        .oneshot(common::get(format!("/tracks/full/{flight_id}")))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/octet-stream"
    );
    assert_eq!(
        resp.headers().get(header::CONTENT_ENCODING).unwrap(),
        "gzip"
    );
    assert_eq!(
        resp.headers().get(header::ETAG).unwrap(),
        format!("\"{etag}\"").as_str()
    );

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        body.as_ref(),
        bytes.as_slice(),
        "body should be the stored bytes verbatim",
    );
    assert_eq!(
        &body[0..2],
        &[0x1f, 0x8b],
        "body must be a gzip stream (Content-Encoding: gzip honesty check)",
    );
}

#[tokio::test]
#[serial]
async fn full_track_returns_304_on_matching_if_none_match() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let flight_id = common::seed_flight(&pool, "NMATCH00", TEST_USER_ID).await;
    let etag = common::seed_full_track(&pool, &flight_id, sample_http_bytes()).await;
    let if_none_match = format!("\"{etag}\"");

    let resp = app
        .oneshot(common::get_if_none_match(
            format!("/tracks/full/{flight_id}"),
            &if_none_match,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(
        resp.headers().get(header::ETAG).unwrap(),
        if_none_match.as_str(),
    );
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert!(body.is_empty(), "304 response must have no body");
}

#[tokio::test]
#[serial]
async fn full_track_unknown_id_returns_404() {
    let (app, _pool) = common::test_app().await;

    let resp = app
        .oneshot(common::get("/tracks/full/NOPE0000"))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
