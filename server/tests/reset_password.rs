//! HTTP integration tests for `POST /users/reset-password` and
//! `POST /users/reset-password/confirm`.
//!
//! There's no SMTP relay in the harness, so the request half asserts on what it
//! *doesn't* do — no stamp, no attempted send — and uses a dead relay to tell
//! "the ladder let this through" apart from "throttled". The confirm half seeds
//! the send list directly and drives the link.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use chrono::{DateTime, Duration, Utc};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;
use serde_json::{Value, json};
use serial_test::serial;
use sqlx::PgPool;
use tengri_server::{
    auth::{Claims, mint_reset_token, password::hash_argon2, token::encode_jwt},
    user::Permissions,
};
use tower::ServiceExt;

const PASSWORD: &str = "hunter2pass";
const NEW_PASSWORD: &str = "brandnewpass9";
const EMAIL: &str = "pilot@example.com";

#[tokio::test]
#[serial]
async fn requesting_is_refused_when_outgoing_mail_is_unconfigured() {
    let (app, pool) = common::test_app().await;
    seed_user(&pool, 30, "resetme", Some(EMAIL)).await;

    let resp = app.oneshot(request_for(EMAIL)).await.unwrap();

    // An install with no relay can't run this flow at all, and saying so leaks
    // nothing — the answer is the same for every address.
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(sends(&pool, 30).await, None);
}

#[tokio::test]
#[serial]
async fn requesting_rejects_a_malformed_address() {
    let (app, pool) = common::test_app().await;
    dead_relay(&pool).await;

    for bad in ["", "not-an-email"] {
        let resp = app.clone().oneshot(request_for(bad)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY, "{bad:?}");
        let fields = body_json(resp).await["fields"].clone();
        assert!(fields["email"].is_string(), "{bad:?} got {fields}");
    }
}

#[tokio::test]
#[serial]
async fn requesting_for_an_address_nobody_holds_says_nothing() {
    let (app, pool) = common::test_app().await;
    dead_relay(&pool).await;
    seed_user(&pool, 31, "resetme", Some(EMAIL)).await;

    // 204 rather than a 404: the form must not double as a way to find out who
    // has an account here. The dead relay is what proves no send was tried —
    // an attempted one would surface as a 5xx.
    let resp = app
        .oneshot(request_for("stranger@example.com"))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(sends(&pool, 31).await, None);
}

#[tokio::test]
#[serial]
async fn requesting_ignores_an_address_still_waiting_on_its_link() {
    let (app, pool) = common::test_app().await;
    dead_relay(&pool).await;
    seed_user(&pool, 32, "pending", None).await;
    sqlx::query("UPDATE users SET pending_email = $1 WHERE id = 32")
        .bind(EMAIL)
        .execute(&pool)
        .await
        .unwrap();

    // Mailing a reset link to an unproven address would let whoever typed it
    // into someone else's signup form take over that account.
    let resp = app.oneshot(request_for(EMAIL)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(sends(&pool, 32).await, None);
}

#[tokio::test]
#[serial]
async fn requesting_ignores_a_disabled_account() {
    let (app, pool) = common::test_app().await;
    dead_relay(&pool).await;
    seed_user(&pool, 33, "banned", Some(EMAIL)).await;
    sqlx::query("UPDATE users SET permissions = 0 WHERE id = 33")
        .execute(&pool)
        .await
        .unwrap();

    let resp = app.oneshot(request_for(EMAIL)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(sends(&pool, 33).await, None);
}

#[tokio::test]
#[serial]
async fn a_rejected_send_leaves_no_wait_behind() {
    let (app, pool) = common::test_app().await;
    dead_relay(&pool).await;
    seed_user(&pool, 34, "resetme", Some(EMAIL)).await;

    let resp = app.oneshot(request_for(EMAIL)).await.unwrap();

    assert!(resp.status().is_server_error() || resp.status().is_client_error());
    assert_eq!(
        sends(&pool, 34).await,
        None,
        "charging a wait for a mail that never left would strand the user"
    );
}

#[tokio::test]
#[serial]
async fn a_second_request_inside_the_wait_sends_nothing() {
    let (app, pool) = common::test_app().await;
    dead_relay(&pool).await;
    seed_user(&pool, 35, "resetme", Some(EMAIL)).await;
    let sent = Utc::now() - Duration::minutes(5);
    set_sends(&pool, 35, &[sent]).await;

    let resp = app.oneshot(request_for(EMAIL)).await.unwrap();

    // Throttled requests answer like every other one. Reaching the dead relay
    // instead would have failed the response, so a 204 here is what says the
    // send was skipped.
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        sends(&pool, 35).await.as_deref(),
        Some(&[sent][..]),
        "a throttled request must not extend the ladder"
    );
}

#[tokio::test]
#[serial]
async fn a_request_after_the_wait_is_let_through() {
    let (app, pool) = common::test_app().await;
    dead_relay(&pool).await;
    seed_user(&pool, 36, "resetme", Some(EMAIL)).await;
    // Two sends an hour apart means a two-hour wait, and it's long past.
    set_sends(
        &pool,
        36,
        &[
            Utc::now() - Duration::hours(6),
            Utc::now() - Duration::hours(5),
        ],
    )
    .await;

    let resp = app.oneshot(request_for(EMAIL)).await.unwrap();

    // Tripping over the dead relay is the signal: the ladder let it through and
    // the handler got as far as trying to send.
    assert!(
        resp.status().is_server_error() || resp.status().is_client_error(),
        "expected the send to be attempted, got {}",
        resp.status()
    );
}

#[tokio::test]
#[serial]
async fn confirming_sets_the_password_and_signs_the_user_in() {
    let (app, pool) = common::test_app().await;
    seed_user(&pool, 37, "resetme", Some(EMAIL)).await;
    let token = arm_link(&pool, 37).await;

    let resp = app
        .clone()
        .oneshot(confirm_with(&token, NEW_PASSWORD))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(session_cookie(&resp).is_some(), "confirming signs them in");
    let body = body_json(resp).await;
    assert_eq!(body["id"], 37);
    assert_eq!(body["has_password"], true);

    assert_eq!(
        sends(&pool, 37).await.as_deref(),
        Some(&[][..]),
        "spending the link clears the ladder, so a new one can be asked for"
    );
    assert!(
        valid_from(&pool, 37).await.is_some(),
        "every other session is done"
    );

    // The new password is the one that works now.
    assert_eq!(login(&app, "resetme", NEW_PASSWORD).await, StatusCode::OK);
    assert_eq!(
        login(&app, "resetme", PASSWORD).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[serial]
async fn confirming_twice_refuses_the_second_click() {
    let (app, pool) = common::test_app().await;
    seed_user(&pool, 38, "resetme", Some(EMAIL)).await;
    let token = arm_link(&pool, 38).await;

    let first = app
        .clone()
        .oneshot(confirm_with(&token, NEW_PASSWORD))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    // The same URL sits in the mailbox, in browser history, and in whatever
    // scanner touched it — a signature and an `exp` alone would keep honouring
    // it for a day.
    let second = app
        .oneshot(confirm_with(&token, "yetanotherpass1"))
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_json(second).await["error"], "reset_link_used");
}

#[tokio::test]
#[serial]
async fn a_newer_link_retires_the_older_one() {
    let (app, pool) = common::test_app().await;
    seed_user(&pool, 39, "resetme", Some(EMAIL)).await;
    let first_sent = Utc::now() - Duration::hours(3);
    let second_sent = Utc::now();
    set_sends(&pool, 39, &[first_sent, second_sent]).await;

    let stale = mint(39, first_sent);
    let resp = app
        .clone()
        .oneshot(confirm_with(&stale, NEW_PASSWORD))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_json(resp).await["error"], "reset_link_used");

    let fresh = mint(39, second_sent);
    let resp = app
        .oneshot(confirm_with(&fresh, NEW_PASSWORD))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn changing_the_password_elsewhere_kills_an_outstanding_link() {
    let (app, pool) = common::test_app().await;
    seed_user(&pool, 40, "resetme", Some(EMAIL)).await;
    let token = arm_link(&pool, 40).await;

    // The owner remembered their password and changed it from the settings page
    // while the reset mail was still in the inbox.
    let resp = app
        .clone()
        .oneshot(common::json_post_with_cookie(
            "/users/me/password",
            json!({ "current_password": PASSWORD, "new_password": NEW_PASSWORD }),
            &common::auth_cookie(40, "resetme"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        session_cookie(&resp).is_some(),
        "the session that changed it gets a cookie minted past the stamp"
    );

    let resp = app
        .oneshot(confirm_with(&token, "athirdpassword7"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_json(resp).await["error"], "reset_link_used");
}

#[tokio::test]
#[serial]
async fn confirming_on_a_disabled_account_changes_nothing() {
    let (app, pool) = common::test_app().await;
    seed_user(&pool, 41, "banned", Some(EMAIL)).await;
    let token = arm_link(&pool, 41).await;
    sqlx::query("UPDATE users SET permissions = 0 WHERE id = 41")
        .execute(&pool)
        .await
        .unwrap();

    let resp = app
        .oneshot(confirm_with(&token, NEW_PASSWORD))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(session_cookie(&resp).is_none(), "and no session either");
    assert_eq!(body_json(resp).await["error"], "account_disabled");
    assert!(
        sends(&pool, 41).await.is_some_and(|list| list.len() == 1),
        "the refusal comes before any write"
    );
}

#[tokio::test]
#[serial]
async fn confirming_rejects_a_password_that_is_too_weak() {
    let (app, pool) = common::test_app().await;
    seed_user(&pool, 42, "resetme", Some(EMAIL)).await;
    let token = arm_link(&pool, 42).await;

    let resp = app.oneshot(confirm_with(&token, "short")).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let fields = body_json(resp).await["fields"].clone();
    assert!(fields["password"].is_string(), "got {fields}");
    assert!(
        sends(&pool, 42).await.is_some_and(|list| list.len() == 1),
        "a rejected password leaves the link usable"
    );
}

#[tokio::test]
#[serial]
async fn confirming_rejects_tokens_it_did_not_mint_for_this() {
    let (app, pool) = common::test_app().await;
    seed_user(&pool, 43, "resetme", Some(EMAIL)).await;
    let sent = Utc::now();
    set_sends(&pool, 43, &[sent]).await;
    let key = EncodingKey::from_secret(common::TEST_JWT_SECRET);

    let expired = raw_token(&RawReset {
        sub: 43,
        stamp: sent.timestamp_micros(),
        kind: "reset-password",
        exp: 1_400_000_000,
    });
    // Signed and unexpired, but minted for a different purpose. Sharing one
    // kind enum across the token flavours is what this guards against.
    let wrong_kind = raw_token(&RawReset {
        sub: 43,
        stamp: sent.timestamp_micros(),
        kind: "confirm-email",
        exp: FAR_FUTURE,
    });
    let session = encode_jwt(
        &Claims::new(
            43,
            "Reset Pilot".into(),
            Permissions::CAN_AUTHORIZE,
            Utc::now().timestamp(),
        ),
        &key,
    )
    .unwrap();
    let unknown_user = mint(99, sent);

    for (label, token, code) in [
        ("expired", expired, "reset_link_expired"),
        ("wrong kind", wrong_kind, "reset_link_used"),
        ("session token", session, "reset_link_used"),
        ("garbage", "not.a.jwt".to_owned(), "reset_link_used"),
        ("unknown user", unknown_user, "reset_link_used"),
    ] {
        let resp = app
            .clone()
            .oneshot(confirm_with(&token, NEW_PASSWORD))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{label}");
        assert_eq!(body_json(resp).await["error"], code, "{label}");
    }

    assert_eq!(
        login(&app, "resetme", PASSWORD).await,
        StatusCode::OK,
        "the password never moved"
    );
}

/// Seed a user with a known password. `email` goes into the proven column,
/// which is the only one a reset link is ever mailed to.
async fn seed_user(pool: &PgPool, id: i32, login: &str, email: Option<&str>) {
    let hash = hash_argon2(PASSWORD).expect("hash test password");
    sqlx::query(
        "INSERT INTO users \
            (id, name, login, email, password_hash, source, permissions) \
         VALUES ($1, $2, $3, $4, $5, 'internal', 1) \
         ON CONFLICT (id) DO UPDATE SET \
             name          = EXCLUDED.name, \
             login         = EXCLUDED.login, \
             email         = EXCLUDED.email, \
             password_hash = EXCLUDED.password_hash, \
             permissions   = EXCLUDED.permissions",
    )
    .bind(id)
    .bind(format!("Pilot {id}"))
    .bind(login)
    .bind(email)
    .bind(&hash)
    .execute(pool)
    .await
    .expect("seed user");
}

/// An SMTP host with nothing listening on it, so any attempted send fails at
/// connect time. Lets a test tell "the handler tried to mail" from "it didn't".
async fn dead_relay(pool: &PgPool) {
    sqlx::query(
        "UPDATE site_settings SET \
            smtp_host = '127.0.0.1', smtp_port = 1, smtp_tls = 'none', \
            from_address = 'noreply@example.com'",
    )
    .execute(pool)
    .await
    .expect("point at a dead relay");
}

/// Stand in for a successful `POST /users/reset-password`, which can't run in
/// the harness: record one send and hand back the link it would have mailed.
async fn arm_link(pool: &PgPool, user_id: i32) -> String {
    let sent = Utc::now();
    set_sends(pool, user_id, &[sent]).await;
    mint(user_id, sent)
}

async fn set_sends(pool: &PgPool, user_id: i32, list: &[DateTime<Utc>]) {
    sqlx::query("UPDATE users SET password_reset_sends = $1 WHERE id = $2")
        .bind(list)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("set the send list");
}

async fn sends(pool: &PgPool, user_id: i32) -> Option<Vec<DateTime<Utc>>> {
    sqlx::query_scalar("SELECT password_reset_sends FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("read the send list")
}

async fn valid_from(pool: &PgPool, user_id: i32) -> Option<DateTime<Utc>> {
    sqlx::query_scalar("SELECT sessions_valid_from FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("read sessions_valid_from")
}

fn mint(user_id: i32, sent: DateTime<Utc>) -> String {
    let key = EncodingKey::from_secret(common::TEST_JWT_SECRET);
    mint_reset_token(user_id, sent.timestamp_micros(), &key).expect("mint reset token")
}

/// Hand-rolled twin of the private `ResetClaims`, so tests can mint the shapes
/// the real minter refuses to make: an expired one, and one tagged for
/// something other than a reset.
#[derive(Serialize)]
struct RawReset {
    sub: i32,
    stamp: i64,
    kind: &'static str,
    exp: i64,
}

fn raw_token(claims: &RawReset) -> String {
    let key = EncodingKey::from_secret(common::TEST_JWT_SECRET);
    encode(&Header::new(Algorithm::HS256), claims, &key).expect("mint raw token")
}

/// Year 2100 — far enough out that `exp` stays valid without the test baking in
/// a "today" that goes stale.
const FAR_FUTURE: i64 = 4_102_444_800;

fn request_for(email: &str) -> Request<Body> {
    json_post("/users/reset-password", json!({ "email": email }))
}

fn confirm_with(token: &str, password: &str) -> Request<Body> {
    json_post(
        "/users/reset-password/confirm",
        json!({ "token": token, "password": password }),
    )
}

async fn login(app: &axum::Router, login: &str, password: &str) -> StatusCode {
    app.clone()
        .oneshot(json_post(
            "/users/login",
            json!({ "identifier": login, "password": password }),
        ))
        .await
        .unwrap()
        .status()
}

fn json_post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn session_cookie(resp: &axum::response::Response) -> Option<String> {
    resp.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("tengri-jwt="))
        .map(str::to_owned)
}
