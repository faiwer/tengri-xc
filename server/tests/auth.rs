//! HTTP integration tests for `/users/login`, `/users/me`,
//! `/users/logout`. Exercises the full cookie + JWT round-trip
//! and the phpass → argon2 rehash on first login.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use jsonwebtoken::EncodingKey;
use md5::{Digest, Md5};
use serde_json::{Value, json};
use serial_test::serial;
use sqlx::Row;
use tengri_server::{
    auth::{Claims, cookie::SLIDE_INTERVAL},
    user::Permissions,
};
use tower::ServiceExt;

const ITOA64: &[u8; 64] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Build a phpass `$H$` hash for the given password using the
/// supplied salt (8 bytes) and cost exponent. Mirrors the
/// implementation in `tengri_server::auth::password::verify_phpass`,
/// kept independent here so the test fails loudly if either
/// implementation drifts.
fn phpass_hash(password: &str, salt: &[u8; 8], cost: u8) -> String {
    let mut h = Md5::new();
    h.update(salt);
    h.update(password.as_bytes());
    let mut digest: [u8; 16] = h.finalize().into();
    for _ in 0..(1u64 << cost) {
        let mut h = Md5::new();
        h.update(digest);
        h.update(password.as_bytes());
        digest = h.finalize().into();
    }
    let mut out = String::with_capacity(34);
    out.push_str("$H$");
    out.push(ITOA64[cost as usize] as char);
    out.push_str(std::str::from_utf8(salt).unwrap());
    out.push_str(&phpass_encode(&digest));
    out
}

fn phpass_encode(input: &[u8; 16]) -> String {
    let mut out = String::with_capacity(22);
    let mut i = 0;
    let count = input.len();
    loop {
        let mut value = input[i] as u32;
        i += 1;
        out.push(ITOA64[(value & 0x3f) as usize] as char);
        if i < count {
            value |= (input[i] as u32) << 8;
        }
        out.push(ITOA64[((value >> 6) & 0x3f) as usize] as char);
        if i >= count {
            break;
        }
        i += 1;
        if i < count {
            value |= (input[i] as u32) << 16;
        }
        out.push(ITOA64[((value >> 12) & 0x3f) as usize] as char);
        if i >= count {
            break;
        }
        i += 1;
        out.push(ITOA64[((value >> 18) & 0x3f) as usize] as char);
        if i >= count {
            break;
        }
    }
    out
}

async fn seed_login_user(
    pool: &sqlx::PgPool,
    id: i32,
    name: &str,
    login: &str,
    email: Option<&str>,
    password_hash: &str,
) {
    sqlx::query(
        "INSERT INTO users (id, name, login, email, password_hash, source, permissions) \
         VALUES ($1, $2, $3, $4, $5, 'leo', 1)",
    )
    .bind(id)
    .bind(name)
    .bind(login)
    .bind(email)
    .bind(password_hash)
    .execute(pool)
    .await
    .expect("seed login user");
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

/// Pull the `Set-Cookie` header that names our session cookie, if
/// present. Tests use this both to feed the token into a follow-
/// up request and to assert the cookie is being cleared.
fn session_set_cookie(resp: &axum::response::Response) -> Option<String> {
    resp.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("tengri-jwt="))
        .map(|s| s.to_owned())
}

/// Extract just the `name=value` portion of a `Set-Cookie` so we
/// can echo it back in `Cookie:` headers (browsers do the same).
fn cookie_pair_from_set_cookie(set_cookie: &str) -> &str {
    set_cookie
        .split(';')
        .next()
        .expect("set-cookie always has at least the name=value part")
}

#[tokio::test]
#[serial]
async fn login_with_bad_credentials_returns_401() {
    let (app, _pool) = common::test_app().await;

    let resp = app
        .oneshot(json_post(
            "/users/login",
            json!({ "identifier": "nobody", "password": "x" }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn login_with_phpass_rehashes_to_argon2_and_round_trips_me() {
    let (app, pool) = common::test_app().await;

    let salt = b"abcdefgh";
    let stored_phpass = phpass_hash("hunter2", salt, 8);
    assert!(stored_phpass.starts_with("$H$"));

    seed_login_user(
        &pool,
        1,
        "Test Pilot",
        "TestPilot",
        Some("test@example.com"),
        &stored_phpass,
    )
    .await;

    // 1. Log in with the right password.
    let resp = app
        .clone()
        .oneshot(json_post(
            "/users/login",
            json!({ "identifier": "testpilot", "password": "hunter2" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = session_set_cookie(&resp).expect("session cookie");
    let body = body_json(resp).await;
    assert_eq!(body["id"], 1);
    assert_eq!(body["name"], "Test Pilot");
    assert_eq!(body["login"], "TestPilot");
    assert_eq!(body["email"], "test@example.com");
    assert_eq!(body["source"], "leo");
    assert_eq!(body["permissions"], 1);

    // 2. The stored hash is now argon2.
    let row = sqlx::query("SELECT password_hash FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let new_hash: String = row.try_get("password_hash").unwrap();
    assert!(
        new_hash.starts_with("$argon2"),
        "expected rehash to argon2, got {new_hash:?}"
    );

    // 3. The cookie is good for `/users/me`.
    let me_req = Request::builder()
        .method("GET")
        .uri("/users/me")
        .header(header::COOKIE, cookie_pair_from_set_cookie(&cookie))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(me_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let me = body_json(resp).await;
    assert_eq!(me["id"], 1);
    assert_eq!(me["login"], "TestPilot");

    // 4. Same login works again with the *new* (argon2) hash and
    //    no longer triggers a rehash.
    let resp = app
        .clone()
        .oneshot(json_post(
            "/users/login",
            json!({ "identifier": "TESTPILOT", "password": "hunter2" }), // case-insensitive
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let row = sqlx::query("SELECT password_hash FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let after_second_login: String = row.try_get("password_hash").unwrap();
    assert_eq!(
        after_second_login, new_hash,
        "argon2 hash should not change across logins"
    );
}

#[tokio::test]
#[serial]
async fn login_by_email_works_and_is_case_insensitive() {
    let (app, pool) = common::test_app().await;

    let stored = phpass_hash("hunter2", b"saltsalt", 8);
    seed_login_user(
        &pool,
        2,
        "Email User",
        "emailuser",
        Some("foo@bar.com"),
        &stored,
    )
    .await;

    let resp = app
        .oneshot(json_post(
            "/users/login",
            // Mixed-case input; users.email is stored lowercase
            // and the SQL `email = LOWER($1)` does the matching.
            json!({ "identifier": "Foo@Bar.COM", "password": "hunter2" }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn me_without_cookie_returns_null() {
    // `/users/me` is a query about identity, and "nobody" is a
    // valid answer. Returning 401 would force every anonymous
    // SPA boot to log a red error in the browser console — and
    // logically, an anonymous user *isn't* a failed request, so
    // the response shape carries `null` and the status stays 200.
    let (app, _pool) = common::test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/users/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert!(
        body.is_null(),
        "expected null body for anonymous /me, got {body}"
    );
}

#[tokio::test]
#[serial]
async fn logout_clears_cookie() {
    let (app, _pool) = common::test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/users/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let set = session_set_cookie(&resp).expect("logout sets a clearing cookie");
    assert!(
        set.contains("Max-Age=0"),
        "logout cookie should expire immediately, got {set:?}"
    );
}

/// Same dummy secret as `common::test_app`. Test helpers that
/// hand-craft JWTs sign with this key so the verifier inside the
/// app accepts them.
const TEST_JWT_SECRET: &[u8; 32] = &[0u8; 32];

/// Build a `Cookie:` header value carrying a freshly-signed JWT
/// for `user_id`, with `iat` shifted backwards by `aged_secs`.
/// Lets tests put the cookie into either branch of
/// [`cookie::is_due_for_slide`] without waiting around in real
/// time.
fn make_cookie_with_aged_iat(user_id: i32, name: &str, aged_secs: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims::new(
        user_id,
        name.to_owned(),
        Permissions::CAN_AUTHORIZE,
        now - aged_secs,
    );
    let key = EncodingKey::from_secret(TEST_JWT_SECRET);
    let jwt = tengri_server::auth::token::encode_jwt(&claims, &key).unwrap();
    format!("tengri-jwt={jwt}")
}

#[tokio::test]
#[serial]
async fn me_does_not_slide_a_fresh_cookie() {
    // The slide threshold is 15 min; a cookie minted seconds ago
    // shouldn't trigger a re-mint. Bandwidth/cookie-jar churn
    // matters more than a fresh `iat` for chatty SPAs.
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 7, "Fresh").await;
    sqlx::query("UPDATE users SET login = 'fresh', source = 'leo', permissions = 1 WHERE id = 7")
        .execute(&pool)
        .await
        .unwrap();

    let cookie = make_cookie_with_aged_iat(7, "Fresh", 30);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/users/me")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        session_set_cookie(&resp).is_none(),
        "fresh cookie should pass through unchanged; got Set-Cookie unexpectedly"
    );
}

#[tokio::test]
#[serial]
async fn me_slides_a_stale_cookie() {
    // An aged cookie (older than `SLIDE_INTERVAL`) gets
    // re-minted: same `sub`/`name`/`p`, fresh `iat`/`exp`. The
    // browser then keeps using the new token going forward, so
    // an active user's window slides indefinitely.
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 8, "Stale").await;
    sqlx::query("UPDATE users SET login = 'stale', source = 'leo', permissions = 1 WHERE id = 8")
        .execute(&pool)
        .await
        .unwrap();

    let aged = SLIDE_INTERVAL.as_secs() as i64 + 60;
    let cookie = make_cookie_with_aged_iat(8, "Stale", aged);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/users/me")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let set = session_set_cookie(&resp).expect("stale cookie should be slid");
    let new_pair = cookie_pair_from_set_cookie(&set);
    assert_ne!(
        new_pair, cookie,
        "slide should mint a new JWT, not echo the input"
    );
    assert!(
        set.contains("HttpOnly"),
        "slid cookie keeps the `HttpOnly` flag, got {set:?}"
    );
}

#[tokio::test]
#[serial]
async fn me_with_stale_cookie_for_revoked_user_clears_cookie_and_returns_null() {
    // The cold-path slide does a DB check. If the user lost their
    // CAN_AUTHORIZE bit (banned, deactivated) since the JWT was
    // minted, the middleware must:
    //   - clear the cookie (Max-Age=0)
    //   - drop the identity from extensions, so `/users/me`
    //     responds with `null`, not the cached token data.
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 9, "Revoked").await;
    sqlx::query("UPDATE users SET login = 'revoked', source = 'leo', permissions = 0 WHERE id = 9")
        .execute(&pool)
        .await
        .unwrap();

    let aged = SLIDE_INTERVAL.as_secs() as i64 + 60;
    let cookie = make_cookie_with_aged_iat(9, "Revoked", aged);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/users/me")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let set = session_set_cookie(&resp).expect("revoked user should get a clearing cookie");
    assert!(
        set.contains("Max-Age=0"),
        "revocation should set Max-Age=0, got {set:?}"
    );
    let body = body_json(resp).await;
    assert!(
        body.is_null(),
        "revoked user's /me should be null, got {body}"
    );
}

#[tokio::test]
#[serial]
async fn me_with_stale_cookie_for_missing_user_clears_cookie_and_returns_null() {
    // Same revocation path as above, except the user row was
    // deleted entirely instead of having its bit cleared. We
    // never seed a row for id=99 — the cookie is signed correctly
    // but points at a row that doesn't exist, exactly the
    // hard-delete case.
    let (app, _pool) = common::test_app().await;

    let aged = SLIDE_INTERVAL.as_secs() as i64 + 60;
    let cookie = make_cookie_with_aged_iat(99, "Ghost", aged);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/users/me")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let set = session_set_cookie(&resp).expect("missing user should get a clearing cookie");
    assert!(set.contains("Max-Age=0"));
    let body = body_json(resp).await;
    assert!(
        body.is_null(),
        "ghost user's /me should be null, got {body}"
    );
}

#[tokio::test]
#[serial]
async fn login_with_cleared_can_authorize_bit_returns_401() {
    let (app, pool) = common::test_app().await;

    let stored = phpass_hash("hunter2", b"abcdefgh", 8);
    sqlx::query(
        "INSERT INTO users (id, name, login, password_hash, source, permissions) \
         VALUES (3, 'Banned', 'banned', $1, 'leo', 0)",
    )
    .bind(&stored)
    .execute(&pool)
    .await
    .unwrap();

    let resp = app
        .oneshot(json_post(
            "/users/login",
            json!({ "identifier": "banned", "password": "hunter2" }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
