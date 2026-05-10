//! HTTP integration tests for `PATCH /users/me`. Covers the
//! self-edit envelope: preferences-only writes, profile-only writes,
//! per-field 422 validation, and the transactional "all or nothing"
//! property when one section validates and another doesn't.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use serial_test::serial;
use sqlx::Row;
use tower::ServiceExt;

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
#[serial]
async fn patch_me_without_session_returns_401() {
    let (app, _pool) = common::test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/users/me")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "preferences": {} }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn patch_me_empty_body_is_400() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({}),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn patch_me_writes_preferences_and_returns_updated_me() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .clone()
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({
                "preferences": {
                    "units": "imperial",
                    "vario_unit": "fpm",
                    "time_format": "h12"
                }
            }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    // Response is the updated `MeDto` — the FE swaps it into the
    // identity context wholesale, so the round-trip needs to be
    // accurate here.
    assert_eq!(body["id"], 1);
    assert_eq!(body["preferences"]["units"], "imperial");
    assert_eq!(body["preferences"]["vario_unit"], "fpm");
    assert_eq!(body["preferences"]["time_format"], "h12");
    // Untouched fields stay on their default.
    assert_eq!(body["preferences"]["date_format"], "system");

    // And the DB actually has the values (catches the write going
    // through a transaction that never commits).
    let row = sqlx::query(
        "SELECT units, vario_unit, time_format FROM user_preferences WHERE user_id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.try_get::<String, _>("units").unwrap(), "imperial");
    assert_eq!(row.try_get::<String, _>("vario_unit").unwrap(), "fpm");
    assert_eq!(row.try_get::<String, _>("time_format").unwrap(), "h12");
}

#[tokio::test]
#[serial]
async fn patch_me_writes_profile_and_upserts_when_no_row() {
    // No user_profiles row exists yet — the apply path must UPSERT
    // rather than UPDATE-zero-rows.
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({
                "profile": { "country": "de", "civl_id": 12345 }
            }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    // Country is auto-uppercased — the validator stores the
    // normalised form so the DB never carries lowercase.
    assert_eq!(body["profile"]["country"], "DE");
    assert_eq!(body["profile"]["civl_id"], 12345);
    // Sex wasn't sent → stays NULL.
    assert!(body["profile"]["sex"].is_null());
}

#[tokio::test]
#[serial]
async fn patch_me_clears_profile_field_with_explicit_null() {
    // First seed a row with a CIVL id, then clear it via PATCH null.
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    sqlx::query("INSERT INTO user_profiles (user_id, civl_id, country) VALUES (1, 42, 'DE')")
        .execute(&pool)
        .await
        .unwrap();
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({ "profile": { "civl_id": null } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    // civl_id cleared, country untouched.
    assert!(body["profile"]["civl_id"].is_null());
    assert_eq!(body["profile"]["country"], "DE");
}

#[tokio::test]
#[serial]
async fn patch_me_returns_per_field_errors_with_namespaced_paths() {
    // Two bad fields across two sections — the response should
    // surface both under their section-prefixed names so the FE
    // can drive AntD's `Form.setFields` in one go.
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({
                "profile": {
                    "country": "Germany",   // not 2 letters
                    "civl_id": -1            // not positive
                }
            }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_json(resp).await;
    assert_eq!(body["error"], "validation");
    assert!(body["fields"]["profile.country"].is_string());
    assert!(body["fields"]["profile.civl_id"].is_string());
}

#[tokio::test]
#[serial]
async fn patch_me_validation_failure_does_not_partially_apply() {
    // Profile validates clean, preferences would too — but we send a
    // bad country to force the request to 422 *before* anything
    // writes. The pre-existing preferences row must be unchanged.
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({
                "profile": { "country": "X" },
                "preferences": { "units": "imperial" }
            }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Preferences row is still all-default — the validation failure
    // ran before any DB write, so the preferences "imperial" never
    // landed.
    let row = sqlx::query("SELECT units FROM user_preferences WHERE user_id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.try_get::<String, _>("units").unwrap(), "system");
}
