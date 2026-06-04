//! HTTP integration tests for `POST /tracks/peek`.

mod common;

use std::io::{Cursor, Write};

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use bincode::config::standard;
use http_body_util::BodyExt;
use serde_json::Value;
use serial_test::serial;
use tengri_server::flight::{TengriFile, decode, ingest::gzip_bytes};
use tower::ServiceExt;
use zip::{ZipWriter, write::SimpleFileOptions};

const TEST_USER_ID: i32 = 1;
const TEST_USER_NAME: &str = "Test Pilot";
const BOUNDARY: &str = "----tengri-peek-test-boundary";
const MAX_DECOMPRESSED_FLIGHT_BYTES: usize = 32 * 1024 * 1024;

#[tokio::test]
#[serial]
async fn peek_scores_and_returns_plain_bincode_flight() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;

    let resp = app
        .oneshot(multipart_request(
            "sample.igc",
            &sample_flying_igc(),
            Some(&common::auth_cookie(TEST_USER_ID, TEST_USER_NAME)),
        ))
        .await
        .unwrap();

    let status = resp.status();
    let json = response_json(resp).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["metadata"]["source_format"], "igc");
    assert!(!json["metadata"]["routes"].as_array().unwrap().is_empty());
    assert!(json["metadata"]["scoring_points"].as_u64().unwrap() >= 5);

    let flight = decode_response_flight(&json);
    let track = decode(&flight.track).expect("decode compact preview track");
    assert_eq!(
        track.points.len() as u64,
        json["metadata"]["flight_points"].as_u64().unwrap()
    );
    assert!(track.points.len() < json["metadata"]["source_points"].as_u64().unwrap() as usize);
    assert_eq!(
        flight.metadata.takeoff_timezone,
        json["metadata"]["takeoff_timezone"].as_str().unwrap()
    );
}

#[tokio::test]
#[serial]
async fn peek_requires_authentication() {
    let (app, _pool) = common::test_app().await;

    let resp = app
        .oneshot(multipart_request("sample.igc", &sample_flying_igc(), None))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn peek_rejects_unsupported_format() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;

    let resp = app
        .oneshot(multipart_request(
            "sample.txt",
            b"not a track",
            Some(&common::auth_cookie(TEST_USER_ID, TEST_USER_NAME)),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn peek_rejects_missing_flight_field() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;

    let resp = app
        .oneshot(multipart_request_with_field(
            "other",
            "sample.igc",
            &sample_flying_igc(),
            Some(&common::auth_cookie(TEST_USER_ID, TEST_USER_NAME)),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn peek_accepts_gzipped_flight_part() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let gz = gzip_bytes(&sample_flying_igc()).expect("gzip sample");

    let resp = app
        .oneshot(multipart_request(
            "sample.igc.gz",
            &gz,
            Some(&common::auth_cookie(TEST_USER_ID, TEST_USER_NAME)),
        ))
        .await
        .unwrap();

    let status = resp.status();
    let json = response_json(resp).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["metadata"]["source_format"], "igc");
    assert!(json["metadata"]["source_points"].as_u64().unwrap() > 0);
}

#[tokio::test]
#[serial]
async fn peek_rejects_gzip_that_inflates_too_large() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let huge = vec![b'A'; MAX_DECOMPRESSED_FLIGHT_BYTES + 1];
    let gz = gzip_bytes(&huge).expect("gzip huge payload");

    let resp = app
        .oneshot(multipart_request(
            "huge.igc.gz",
            &gz,
            Some(&common::auth_cookie(TEST_USER_ID, TEST_USER_NAME)),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn peek_rejects_kmz_that_inflates_too_large() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let kmz = kmz_with_doc_kml(&vec![b' '; MAX_DECOMPRESSED_FLIGHT_BYTES + 1]);

    let resp = app
        .oneshot(multipart_request(
            "huge.kmz",
            &kmz,
            Some(&common::auth_cookie(TEST_USER_ID, TEST_USER_NAME)),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn peek_rejects_tracks_with_too_many_points() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, TEST_USER_ID, TEST_USER_NAME).await;
    let gz = gzip_bytes(&stationary_igc(300_001)).expect("gzip large track");

    let resp = app
        .oneshot(multipart_request(
            "too-many.igc.gz",
            &gz,
            Some(&common::auth_cookie(TEST_USER_ID, TEST_USER_NAME)),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = response_json(resp).await;
    assert!(json["message"].as_str().unwrap().contains("300000"));
}

fn multipart_request(filename: &str, bytes: &[u8], cookie: Option<&str>) -> Request<Body> {
    multipart_request_with_field("flight", filename, bytes, cookie)
}

fn multipart_request_with_field(
    field: &str,
    filename: &str,
    bytes: &[u8],
    cookie: Option<&str>,
) -> Request<Body> {
    let mut body = Vec::new();
    write!(
        body,
        "--{BOUNDARY}\r\n\
         Content-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n"
    )
    .unwrap();
    body.extend_from_slice(bytes);
    write!(body, "\r\n--{BOUNDARY}--\r\n").unwrap();

    let mut builder = Request::builder()
        .method("POST")
        .uri("/tracks/peek")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        );
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    builder.body(Body::from(body)).unwrap()
}

async fn response_json(resp: axum::response::Response) -> Value {
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).expect("response is valid JSON")
}

fn decode_response_flight(json: &Value) -> TengriFile {
    let encoded = json["flight"].as_str().expect("flight is a string");
    let bytes = B64.decode(encoded).expect("flight is base64");
    let (file, _): (TengriFile, _) =
        bincode::serde::decode_from_slice(&bytes, standard()).expect("flight is bincode");
    file
}

fn sample_flying_igc() -> Vec<u8> {
    let mut out = String::from("HFDTE030526\n");

    let mut time = 0;
    for _ in 0..120 {
        out.push_str(&igc_fix(time, 47.0, 8.0, 500));
        time += 1;
    }

    for idx in 0..800 {
        let phase = idx / 200;
        let step = (idx % 200) as f64 * 0.0002;
        let (lat, lon) = match phase {
            0 => (47.0, 8.0 + step),
            1 => (47.0 + step, 8.04),
            2 => (47.04, 8.04 - step),
            _ => (47.04 - step, 8.0),
        };
        out.push_str(&igc_fix(time, lat, lon, 500 + idx));
        time += 1;
    }

    for _ in 0..360 {
        out.push_str(&igc_fix(time, 47.0, 8.0, 500));
        time += 1;
    }
    out.into_bytes()
}

fn stationary_igc(points: usize) -> Vec<u8> {
    let mut out = String::from("HFDTE030526\n");
    for idx in 0..points {
        out.push_str(&igc_fix(idx as u32, 47.0, 8.0, 500));
    }
    out.into_bytes()
}

fn igc_fix(seconds: u32, lat: f64, lon: f64, alt_m: i32) -> String {
    let (lat_value, lat_hemi) = igc_coord(lat, 2, 'N', 'S');
    let (lon_value, lon_hemi) = igc_coord(lon, 3, 'E', 'W');
    format!(
        "B{hh:02}{mm:02}{ss:02}{lat_value}{lat_hemi}{lon_value}{lon_hemi}A{alt:05}{alt:05}\n",
        hh = (seconds / 3600) % 24,
        mm = (seconds / 60) % 60,
        ss = seconds % 60,
        alt = alt_m,
    )
}

fn igc_coord(value: f64, deg_width: usize, positive: char, negative: char) -> (String, char) {
    let hemi = if value >= 0.0 { positive } else { negative };
    let abs = value.abs();
    let mut degrees = abs.floor() as u32;
    let mut minute_milli = ((abs - f64::from(degrees)) * 60_000.0).round() as u32;
    if minute_milli == 60_000 {
        degrees += 1;
        minute_milli = 0;
    }
    (format!("{degrees:0deg_width$}{minute_milli:05}"), hemi)
}

fn kmz_with_doc_kml(kml: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut out));
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("doc.kml", opts).unwrap();
        zip.write_all(kml).unwrap();
        zip.finish().unwrap();
    }
    out
}
