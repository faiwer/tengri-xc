//! HTTP integration tests for `GET /me/flights/{id}/source`.

mod common;

use axum::http::{StatusCode, header};
use http_body_util::BodyExt;
use serial_test::serial;
use sqlx::PgPool;
use tengri_server::{flight::ingest::gzip_bytes, user::Permissions};
use tower::ServiceExt;

const OWNER_ID: i32 = 1;
const OWNER_NAME: &str = "Owner Pilot";
const STRANGER_ID: i32 = 2;
const STRANGER_NAME: &str = "Someone Else";

const FLIGHT_ID: &str = "SRCTEST0";
const SOURCE_BYTES: &[u8] = b"AXXXtest igc\nB1200000000000N000000000EA0000000000\n";

// 2026-11-29T12:00:00Z / +1h. Fixed so the filename assertion is deterministic.
const TAKEOFF_AT: i64 = 1_795_953_600;
const LANDING_AT: i64 = 1_795_957_200;

#[tokio::test]
#[serial]
async fn download_source_returns_original_bytes_as_attachment() {
    let (app, pool) = common::test_app().await;
    seed_flight_with_source(&pool, OWNER_ID, OWNER_NAME).await;

    let resp = app
        .oneshot(common::get_with_header(
            format!("/me/flights/{FLIGHT_ID}/source"),
            header::COOKIE,
            common::auth_cookie(OWNER_ID, OWNER_NAME),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_DISPOSITION).unwrap(),
        "attachment; filename=\"2026-11-29_SRCTEST0.igc\"",
    );
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/octet-stream",
    );

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body.as_ref(), SOURCE_BYTES);
}

/// 23:30 UTC is already the next day in Almaty, so the filename date proves
/// we read the flight's own timezone rather than UTC.
#[tokio::test]
#[serial]
async fn download_source_filename_uses_takeoff_timezone() {
    let (app, pool) = common::test_app().await;
    seed_flight_with_source(&pool, OWNER_ID, OWNER_NAME).await;
    sqlx::query(
        "UPDATE flights \
         SET takeoff_at = to_timestamp($1), takeoff_timezone = 'Asia/Almaty' \
         WHERE id = $2",
    )
    .bind(TAKEOFF_AT + 41_400) // 2026-11-29T23:30:00Z
    .bind(FLIGHT_ID)
    .execute(&pool)
    .await
    .expect("shift takeoff into the next local day");

    let resp = app
        .oneshot(common::get_with_header(
            format!("/me/flights/{FLIGHT_ID}/source"),
            header::COOKIE,
            common::auth_cookie(OWNER_ID, OWNER_NAME),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_DISPOSITION).unwrap(),
        "attachment; filename=\"2026-11-30_SRCTEST0.igc\"",
    );
}

#[tokio::test]
#[serial]
async fn download_source_allowed_with_manage_tracks() {
    let (app, pool) = common::test_app().await;
    seed_flight_with_source(&pool, OWNER_ID, OWNER_NAME).await;
    common::seed_user(&pool, STRANGER_ID, STRANGER_NAME).await;

    let resp = app
        .oneshot(common::get_with_header(
            format!("/me/flights/{FLIGHT_ID}/source"),
            header::COOKIE,
            common::auth_cookie_with_permissions(
                STRANGER_ID,
                STRANGER_NAME,
                Permissions::CAN_AUTHORIZE | Permissions::MANAGE_TRACKS,
            ),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn download_source_forbidden_for_other_user() {
    let (app, pool) = common::test_app().await;
    seed_flight_with_source(&pool, OWNER_ID, OWNER_NAME).await;
    common::seed_user(&pool, STRANGER_ID, STRANGER_NAME).await;

    let resp = app
        .oneshot(common::get_with_header(
            format!("/me/flights/{FLIGHT_ID}/source"),
            header::COOKIE,
            common::auth_cookie(STRANGER_ID, STRANGER_NAME),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn download_source_unknown_flight_returns_404() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, OWNER_ID, OWNER_NAME).await;

    let resp = app
        .oneshot(common::get_with_header(
            "/me/flights/NOPE0000/source",
            header::COOKIE,
            common::auth_cookie(OWNER_ID, OWNER_NAME),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

async fn seed_flight_with_source(pool: &PgPool, user_id: i32, user_name: &str) {
    common::seed_user(pool, user_id, user_name).await;
    common::seed_flight_at(pool, FLIGHT_ID, user_id, TAKEOFF_AT, LANDING_AT).await;

    sqlx::query(
        "INSERT INTO flight_sources (flight_id, format, bytes) \
         VALUES ($1, 'igc'::flight_source_format, $2)",
    )
    .bind(FLIGHT_ID)
    .bind(gzip_bytes(SOURCE_BYTES).expect("gzip fixture source"))
    .execute(pool)
    .await
    .expect("seed flight source");
}
