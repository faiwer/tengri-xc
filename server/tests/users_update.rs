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
async fn patch_me_writes_name_to_users() {
    // Name only: a self-service `email` never lands on `users.email` directly,
    // it goes through the pending-address flow above.
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
    let body = body_json(resp).await;
    assert_eq!(body["name"], "Renamed Pilot");

    let name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "Renamed Pilot");
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

/// Seed `id=1` with a proven address — `users.email` only ever holds those.
async fn seed_verified_email(pool: &sqlx::PgPool, email: &str) {
    common::seed_user(pool, 1, "Pilot").await;
    sqlx::query("UPDATE users SET email = $1 WHERE id = 1")
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
}

/// `(email, pending_email)` for `id=1`.
async fn addresses(pool: &sqlx::PgPool) -> (Option<String>, Option<String>) {
    let row = sqlx::query("SELECT email, pending_email FROM users WHERE id = 1")
        .fetch_one(pool)
        .await
        .unwrap();
    (
        row.try_get("email").unwrap(),
        row.try_get("pending_email").unwrap(),
    )
}

/// Point the SMTP settings at a dead port so a send fails at connect time —
/// enough to prove the handler *tried*, and that the failure rolls back.
async fn set_dead_smtp(pool: &sqlx::PgPool) {
    sqlx::query(
        "UPDATE site_settings SET \
            smtp_host = '127.0.0.1', smtp_port = 1, smtp_tls = 'none', \
            from_address = 'noreply@example.com'",
    )
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
#[serial]
async fn patch_me_email_change_is_refused_when_outgoing_mail_is_unconfigured() {
    // Storing an address we can't mail a link to would strand the change with
    // no way to finish it, so the refusal comes before any write.
    let (app, pool) = common::test_app().await;
    seed_verified_email(&pool, "old@example.com").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({ "profile": { "name": "Renamed", "email": "new@example.com" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(
        addresses(&pool).await,
        (Some("old@example.com".to_owned()), None),
        "neither column may move"
    );
    let name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "Pilot", "the rest of the save goes back too");
}

#[tokio::test]
#[serial]
async fn patch_me_email_change_rolls_back_when_the_relay_rejects_the_mail() {
    let (app, pool) = common::test_app().await;
    seed_verified_email(&pool, "old@example.com").await;
    set_dead_smtp(&pool).await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({ "profile": { "name": "Pilot", "email": "new@example.com" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    assert_eq!(
        addresses(&pool).await,
        (Some("old@example.com".to_owned()), None),
        "a pending address the user was never told about is worse than no change"
    );
}

#[tokio::test]
#[serial]
async fn patch_me_email_change_by_non_admin_never_touches_the_proven_address() {
    // The load-bearing property: a mistyped address must cost one retry, not
    // the account. `email` stays put, so password login keeps working and the
    // user can just save again.
    let (app, pool) = common::test_app().await;
    seed_verified_email(&pool, "old@example.com").await;
    set_dead_smtp(&pool).await;
    let cookie = common::auth_cookie(1, "Pilot");

    let _ = app
        .oneshot(common::json_patch_with_cookie(
            "/users/me",
            json!({ "profile": { "name": "Pilot", "email": "typo@example.com" } }),
            &cookie,
        ))
        .await
        .unwrap();

    assert_eq!(
        addresses(&pool).await.0,
        Some("old@example.com".to_owned()),
        "the proven address stays put, so the account can still sign in"
    );
}

#[tokio::test]
#[serial]
async fn patch_me_same_email_leaves_a_pending_change_alone() {
    // Re-saving the address on file (here only case differs, and the server
    // lowercases both) isn't a change, so it writes nothing. It must not double
    // as "cancel my pending change" either: the form is pre-filled with the
    // proven address, so every name-only edit resubmits it, and cancelling on
    // that would silently drop an address change the user is mid-way through.
    // Clearing the slot belongs to the promote in `GET /users/confirm-email`.
    let (app, pool) = common::test_app().await;
    seed_verified_email(&pool, "keep@example.com").await;
    sqlx::query("UPDATE users SET pending_email = 'typo@example.com' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
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
    // This request mailed no link itself, so it reports none — even though the
    // record it returns still carries the older pending address.
    assert!(body["confirmation_sent_to"].is_null());
    assert_eq!(body["pending_email"], "typo@example.com");
    assert_eq!(
        addresses(&pool).await,
        (
            Some("keep@example.com".to_owned()),
            Some("typo@example.com".to_owned())
        ),
        "the in-flight change survives an edit that didn't touch the address"
    );
}

#[tokio::test]
#[serial]
async fn patch_me_email_change_by_admin_goes_pending_like_anyone_elses() {
    // MANAGE_USERS buys no shortcut here. `email` means "somebody proved this",
    // and `find_user_id_by_email` hands OAuth sign-ins to whoever holds it, so
    // an address the owner typed but never confirmed can't go there. An admin
    // who wants a no-proof write uses `/admin/users`, where typing the address
    // is itself the proof. So this refuses for want of a relay, exactly as it
    // would for a regular owner.
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

    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(
        addresses(&pool).await,
        (Some("old@example.com".to_owned()), None),
        "no write-through, so `email` still holds the proven address"
    );
}
