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
use jsonwebtoken::DecodingKey;
use serde_json::{Value, json};
use serial_test::serial;
use sqlx::Row;
use tengri_server::auth::token::decode_jwt;
use tengri_server::user::Permissions;
use tower::ServiceExt;

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Pull the `tengri-jwt` value out of a response's `Set-Cookie` header, or
/// `None` if the handler didn't reissue the session cookie.
fn session_jwt_from(resp: &axum::response::Response) -> Option<String> {
    let raw = resp.headers().get(header::SET_COOKIE)?.to_str().ok()?;
    raw.split(';')
        .next()?
        .trim()
        .strip_prefix("tengri-jwt=")
        .map(str::to_owned)
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

#[tokio::test]
#[serial]
async fn patch_me_writes_name_and_email_to_users() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            // Email is mixed-case on the wire — the server lowercases it.
            json!({ "profile": { "name": "Renamed Pilot", "email": "Renamed@Example.com" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["name"], "Renamed Pilot");
    assert_eq!(body["email"], "renamed@example.com");

    let row = sqlx::query("SELECT name, email FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.try_get::<String, _>("name").unwrap(), "Renamed Pilot");
    assert_eq!(
        row.try_get::<String, _>("email").unwrap(),
        "renamed@example.com"
    );
}

#[tokio::test]
#[serial]
async fn patch_me_name_change_reissues_session_cookie() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({ "profile": { "name": "Renamed Pilot" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    // The JWT caches the display name; a name change must re-mint it so
    // the navbar doesn't stay stale until the next slide.
    let jwt = session_jwt_from(&resp).expect("name change reissues the session cookie");
    let key = DecodingKey::from_secret(common::TEST_JWT_SECRET);
    let claims = decode_jwt(&jwt, &key).unwrap();
    assert_eq!(claims.name, "Renamed Pilot");
}

#[tokio::test]
#[serial]
async fn patch_me_unchanged_name_does_not_reissue_cookie() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            // Same name as the session — no cookie churn expected.
            json!({ "profile": { "name": "Pilot", "country": "de" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(session_jwt_from(&resp).is_none());
}

#[tokio::test]
#[serial]
async fn patch_me_rejects_name_and_email_already_taken() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    // A second user owns "Taken" and the address; user 1 tries to grab both.
    sqlx::query("INSERT INTO users (id, name, email) VALUES (2, 'Taken', 'taken@example.com')")
        .execute(&pool)
        .await
        .unwrap();
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            // Name match folds case (`users_name_lower_key`).
            json!({ "profile": { "name": "taken", "email": "taken@example.com" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_json(resp).await;
    assert_eq!(body["error"], "validation");
    assert_eq!(body["fields"]["profile.name"], "Already taken");
    assert_eq!(body["fields"]["profile.email"], "Already taken");
}

#[tokio::test]
#[serial]
async fn patch_me_blank_email_leaves_existing_untouched() {
    // Email is never cleared via self-service — a blank value is a no-op.
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    sqlx::query("UPDATE users SET email = 'keep@example.com' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({ "profile": { "name": "Pilot", "email": "" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["email"], "keep@example.com");

    let row = sqlx::query("SELECT email FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.try_get::<String, _>("email").unwrap(),
        "keep@example.com"
    );
}

/// Seed `id=1` with a verified address, returning the cookie the test uses.
async fn seed_verified_email(pool: &sqlx::PgPool, email: &str) {
    common::seed_user(pool, 1, "Pilot").await;
    sqlx::query("UPDATE users SET email = $1, email_verified_at = now() WHERE id = 1")
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
}

async fn email_unverified(pool: &sqlx::PgPool) -> bool {
    sqlx::query_scalar("SELECT email_verified_at IS NULL FROM users WHERE id = 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
#[serial]
async fn patch_me_email_change_by_non_admin_resets_verification() {
    // A regular owner swapping their address hasn't proven the new one —
    // the confirmation must drop so no stale "verified" badge carries over.
    let (app, pool) = common::test_app().await;
    seed_verified_email(&pool, "old@example.com").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({ "profile": { "name": "Pilot", "email": "new@example.com" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(
        body["email_verification_reset"], true,
        "response must flag the reset so the client can toast"
    );
    assert!(
        email_unverified(&pool).await,
        "changing the address as a non-admin must clear email_verified_at"
    );
}

#[tokio::test]
#[serial]
async fn patch_me_same_email_keeps_verification() {
    // Re-saving the *same* address (here only case differs, and the server
    // lowercases both) isn't a change, so the confirmation must survive.
    let (app, pool) = common::test_app().await;
    seed_verified_email(&pool, "keep@example.com").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({ "profile": { "name": "Pilot", "email": "Keep@Example.com" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["email_verification_reset"], false);
    assert!(
        !email_unverified(&pool).await,
        "an unchanged address must not churn email_verified_at"
    );
}

#[tokio::test]
#[serial]
async fn patch_me_email_change_by_admin_keeps_verification() {
    // A MANAGE_USERS owner is trusted to manage verification explicitly, so
    // even a self-edit that swaps the address leaves the timestamp intact.
    let (app, pool) = common::test_app().await;
    seed_verified_email(&pool, "old@example.com").await;
    let cookie = common::auth_cookie_with_permissions(
        1,
        "Pilot",
        Permissions::CAN_AUTHORIZE | Permissions::MANAGE_USERS,
    );

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({ "profile": { "name": "Pilot", "email": "new@example.com" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["email_verification_reset"], false);
    assert!(
        !email_unverified(&pool).await,
        "an admin's self-edit must not clear email_verified_at"
    );
}
