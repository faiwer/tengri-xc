//! HTTP integration tests for `POST /users/register`, `GET
//! /users/confirm-email`, and the unconfirmed-email gate on password login.
//!
//! There's no SMTP relay in the harness, so the register tests cover the
//! refusals and the rollback; the confirmation half seeds its users directly
//! and drives the link.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;
use serde_json::{Value, json};
use serial_test::serial;
use sqlx::PgPool;
use tengri_server::{
    auth::{Claims, mint_confirm_token, password::hash_argon2, token::encode_jwt},
    user::Permissions,
};
use tower::ServiceExt;

const PASSWORD: &str = "hunter2pass";

#[tokio::test]
#[serial]
async fn register_is_refused_while_the_site_switch_is_off() {
    let (app, pool) = common::test_app().await;
    set_can_register(&pool, false).await;

    let resp = app
        .oneshot(json_post("/users/register", body()))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(count_logins(&pool, "newpilot").await, 0);
}

#[tokio::test]
#[serial]
async fn register_reports_every_field_problem_at_once() {
    let (app, pool) = common::test_app().await;
    set_can_register(&pool, true).await;

    let resp = app
        .oneshot(json_post(
            "/users/register",
            json!({
                "name": "   ",
                "login": "victim@example.com",
                "email": "not-an-email",
                "password": "short",
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let fields = body_json(resp).await["fields"].clone();
    assert!(fields["name"].is_string(), "got {fields}");
    assert!(fields["login"].is_string(), "got {fields}");
    assert!(fields["email"].is_string(), "got {fields}");
    assert!(fields["password"].is_string(), "got {fields}");
}

#[tokio::test]
#[serial]
async fn register_requires_an_email() {
    let (app, pool) = common::test_app().await;
    set_can_register(&pool, true).await;

    let mut input = body();
    input["email"] = json!("");
    let resp = app
        .oneshot(json_post("/users/register", input))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let fields = body_json(resp).await["fields"].clone();
    assert!(fields["email"].is_string(), "got {fields}");
}

#[tokio::test]
#[serial]
async fn register_reports_a_taken_login_or_email() {
    let (app, pool) = common::test_app().await;
    set_can_register(&pool, true).await;
    seed_password_user(&pool, 20, "Existing", "newpilot", Some("new@example.com")).await;

    let resp = app
        .oneshot(json_post("/users/register", body()))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let fields = body_json(resp).await["fields"].clone();
    assert_eq!(fields["login"], "Already taken");
    assert_eq!(fields["email"], "Already taken");
}

#[tokio::test]
#[serial]
async fn register_is_not_blocked_by_an_address_pending_for_someone_else() {
    let (app, pool) = common::test_app().await;
    set_can_register(&pool, true).await;
    // Somebody started a signup for this address and never proved it. Letting
    // that reserve the address would hand anyone a way to take a stranger's
    // email out of circulation from the registration form.
    seed_password_user(&pool, 21, "Squatter", "squatter", None).await;
    set_pending_email(&pool, 21, "new@example.com").await;

    let resp = app
        .oneshot(json_post("/users/register", body()))
        .await
        .unwrap();

    // Reaching the outgoing-mail check (409) rather than the uniqueness one
    // (422 with `fields.email`) is what says the pending address stood aside.
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
#[serial]
async fn register_is_refused_when_outgoing_mail_is_unconfigured() {
    let (app, pool) = common::test_app().await;
    set_can_register(&pool, true).await;

    let resp = app
        .oneshot(json_post("/users/register", body()))
        .await
        .unwrap();

    // An account nobody can confirm is squatting on a login and a name, so the
    // refusal has to come before the insert.
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(count_logins(&pool, "newpilot").await, 0);
}

#[tokio::test]
#[serial]
async fn register_is_refused_when_outgoing_mail_is_half_configured() {
    let (app, pool) = common::test_app().await;
    set_can_register(&pool, true).await;
    // A host on its own isn't enough to send with — `from_address` is just as
    // required, and a blank one counts as absent. Checking only the host let
    // this through to fail mid-transaction instead.
    sqlx::query("UPDATE site_settings SET smtp_host = 'smtp.example.com', from_address = ''")
        .execute(&pool)
        .await
        .unwrap();

    let resp = app
        .oneshot(json_post("/users/register", body()))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(count_logins(&pool, "newpilot").await, 0);
}

#[tokio::test]
#[serial]
async fn register_rolls_the_account_back_when_the_relay_rejects_the_mail() {
    let (app, pool) = common::test_app().await;
    set_can_register(&pool, true).await;
    // Nothing listens on port 1, so the send fails at connect time.
    sqlx::query(
        "UPDATE site_settings SET \
            smtp_host = '127.0.0.1', smtp_port = 1, smtp_tls = 'none', \
            from_address = 'noreply@example.com'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let resp = app
        .oneshot(json_post("/users/register", body()))
        .await
        .unwrap();

    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    assert_eq!(
        count_logins(&pool, "newpilot").await,
        0,
        "a failed send must not leave an unconfirmable account behind"
    );
}

#[tokio::test]
#[serial]
async fn confirming_twice_lands_the_same_way() {
    let (app, pool) = common::test_app().await;
    seed_password_user(&pool, 21, "Fresh Pilot", "fresh", None).await;
    set_pending_email(&pool, 21, "fresh@example.com").await;
    let token = confirm_token(21, "fresh@example.com");

    let resp = app
        .clone()
        .oneshot(common::get(format!("/users/confirm-email?token={token}")))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert!(
        location(&resp).contains("email=confirmed"),
        "{:?}",
        location(&resp)
    );
    assert!(session_cookie(&resp).is_some(), "expected a session cookie");
    let promoted = (Some("fresh@example.com".to_owned()), None);
    assert_eq!(addresses(&pool, 21).await, promoted);

    // A mail scanner that prefetched the link, or a second click. The address
    // is already promoted by now, so the token matches `email` rather than
    // `pending_email` — it still has to land on the same page with a session,
    // not on an error.
    let resp = app
        .oneshot(common::get(format!("/users/confirm-email?token={token}")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert!(location(&resp).contains("email=confirmed"));
    assert!(session_cookie(&resp).is_some());
    assert_eq!(addresses(&pool, 21).await, promoted, "and nothing moved");
}

#[tokio::test]
#[serial]
async fn confirming_a_banned_account_hands_out_no_session() {
    let (app, pool) = common::test_app().await;
    seed_password_user(&pool, 22, "Banned Pilot", "banned", None).await;
    set_pending_email(&pool, 22, "banned@example.com").await;
    sqlx::query("UPDATE users SET permissions = 0 WHERE id = 22")
        .execute(&pool)
        .await
        .unwrap();

    let resp = app
        .oneshot(common::get(format!(
            "/users/confirm-email?token={}",
            confirm_token(22, "banned@example.com")
        )))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert!(location(&resp).contains("email=confirmed"));
    assert!(
        session_cookie(&resp).is_none(),
        "a banned account gets no session"
    );
    assert_eq!(
        addresses(&pool, 22).await,
        (Some("banned@example.com".to_owned()), None),
        "the address is still promoted"
    );
}

#[tokio::test]
#[serial]
async fn confirming_rejects_tokens_it_did_not_mint_for_this() {
    let (app, pool) = common::test_app().await;
    seed_password_user(&pool, 23, "Token Pilot", "tokens", None).await;
    set_pending_email(&pool, 23, "tokens@example.com").await;
    let key = EncodingKey::from_secret(common::TEST_JWT_SECRET);

    let expired = raw_token(&RawConfirm {
        sub: 23,
        email: "tokens@example.com".into(),
        kind: "confirm-email",
        exp: 1_400_000_000,
    });
    let wrong_kind = raw_token(&RawConfirm {
        sub: 23,
        email: "tokens@example.com".into(),
        kind: "session",
        exp: far_future(),
    });
    let session = encode_jwt(
        &Claims::new(
            23,
            "Token Pilot".into(),
            Permissions::CAN_AUTHORIZE,
            chrono::Utc::now().timestamp(),
        ),
        &key,
    )
    .unwrap();
    // Minted for an address the user has since moved away from.
    let stale_address = confirm_token(23, "old@example.com");

    for (label, token, reason) in [
        ("expired", expired, "expired"),
        ("wrong kind", wrong_kind, "invalid"),
        ("session token", session, "invalid"),
        ("garbage", "not.a.jwt".to_owned(), "invalid"),
        ("stale address", stale_address, "invalid"),
    ] {
        let resp = app
            .clone()
            .oneshot(common::get(format!("/users/confirm-email?token={token}")))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{label}");
        assert_eq!(
            location(&resp),
            format!("/?email_error={reason}"),
            "{label}"
        );
        assert!(session_cookie(&resp).is_none(), "{label} must mint nothing");
        assert_eq!(
            addresses(&pool, 23).await,
            (None, Some("tokens@example.com".to_owned())),
            "{label} must not promote"
        );
    }
}

#[tokio::test]
#[serial]
async fn confirming_a_registration_promotes_the_pending_address() {
    let (app, pool) = common::test_app().await;
    // Straight out of `POST /users/register`: a login and a pending address,
    // nothing in `email` yet.
    seed_password_user(&pool, 29, "Fresh Pilot", "fresh", None).await;
    set_pending_email(&pool, 29, "fresh@example.com").await;

    let resp = app
        .oneshot(common::get(format!(
            "/users/confirm-email?token={}",
            confirm_token(29, "fresh@example.com")
        )))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    // "confirmed", not "changed" — there was no address to replace, so the
    // landing toast should welcome them rather than report a swap.
    assert!(
        location(&resp).contains("email=confirmed"),
        "{:?}",
        location(&resp)
    );
    assert!(session_cookie(&resp).is_some(), "confirming signs them in");
    assert_eq!(
        addresses(&pool, 29).await,
        (Some("fresh@example.com".to_owned()), None),
        "the pending address becomes the proven one"
    );
}

#[tokio::test]
#[serial]
async fn confirming_a_pending_address_promotes_it_over_the_old_one() {
    let (app, pool) = common::test_app().await;
    seed_password_user(&pool, 26, "Moving Pilot", "moving", Some("old@example.com")).await;
    set_pending_email(&pool, 26, "new@example.com").await;

    let resp = app
        .oneshot(common::get(format!(
            "/users/confirm-email?token={}",
            confirm_token(26, "new@example.com")
        )))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert!(
        location(&resp).contains("email=changed"),
        "{:?}",
        location(&resp)
    );
    assert!(session_cookie(&resp).is_some());
    assert_eq!(
        addresses(&pool, 26).await,
        (Some("new@example.com".to_owned()), None),
        "the proven address replaces the old one and `pending_email` clears"
    );
}

#[tokio::test]
#[serial]
async fn confirming_a_pending_address_yields_to_whoever_proved_it_first() {
    let (app, pool) = common::test_app().await;
    seed_password_user(&pool, 27, "Slow Pilot", "slow", Some("mine@example.com")).await;
    set_pending_email(&pool, 27, "contested@example.com").await;
    // Someone else confirmed the same address in the meantime.
    seed_password_user(
        &pool,
        28,
        "Quick Pilot",
        "quick",
        Some("contested@example.com"),
    )
    .await;

    let resp = app
        .oneshot(common::get(format!(
            "/users/confirm-email?token={}",
            confirm_token(27, "contested@example.com")
        )))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&resp), "/?email_error=taken");
    assert!(session_cookie(&resp).is_none());
    assert_eq!(
        addresses(&pool, 27).await,
        (
            Some("mine@example.com".to_owned()),
            Some("contested@example.com".to_owned())
        ),
        "the loser keeps their own address, and can retry with another"
    );
}

#[tokio::test]
#[serial]
async fn password_login_waits_for_the_address_to_be_confirmed() {
    let (app, pool) = common::test_app().await;
    // The shape registration leaves behind: nothing proven, one address pending.
    seed_password_user(&pool, 24, "Waiting Pilot", "waiting", None).await;
    set_pending_email(&pool, 24, "wait@example.com").await;

    let resp = app
        .clone()
        .oneshot(json_post(
            "/users/login",
            json!({ "identifier": "waiting", "password": PASSWORD }),
        ))
        .await
        .unwrap();
    // 403, not 401: the password already checked out, and the caller needs to
    // know it's their inbox they should be looking at. The code is what the
    // SPA branches on — a bare `forbidden` would leave it guessing.
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = body_json(resp).await;
    assert_eq!(body["error"], "email_unconfirmed");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| m.contains("inbox")),
        "the message has to say where to look: {body}"
    );

    // What the confirmation click does.
    sqlx::query("UPDATE users SET email = pending_email, pending_email = NULL WHERE id = 24")
        .execute(&pool)
        .await
        .unwrap();

    let resp = app
        .oneshot(json_post(
            "/users/login",
            json!({ "identifier": "waiting", "password": PASSWORD }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn password_login_ignores_the_gate_for_an_admin_set_address() {
    let (app, pool) = common::test_app().await;
    // An admin typed this address straight into `email`, so nothing ever mails
    // a link for it. The gate has to key on a *pending* address, not on the
    // absence of a confirmation, or the account is locked out for good.
    seed_password_user(
        &pool,
        28,
        "Admin Made",
        "adminmade",
        Some("admin-made@example.com"),
    )
    .await;

    let resp = app
        .oneshot(json_post(
            "/users/login",
            json!({ "identifier": "adminmade", "password": PASSWORD }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn password_login_ignores_the_gate_for_an_account_with_no_email() {
    let (app, pool) = common::test_app().await;
    // Admin-created, login-only accounts have nothing to confirm. Gating on
    // "unverified" rather than "has an unverified address" would lock them out.
    seed_password_user(&pool, 25, "Login Only", "loginonly", None).await;

    let resp = app
        .oneshot(json_post(
            "/users/login",
            json!({ "identifier": "loginonly", "password": PASSWORD }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

/// A registration that would succeed if mail were configured. Individual tests
/// break one field at a time.
fn body() -> Value {
    json!({
        "name": "New Pilot",
        "login": "newpilot",
        "email": "new@example.com",
        "password": PASSWORD,
    })
}

async fn set_can_register(pool: &PgPool, enabled: bool) {
    sqlx::query("UPDATE site_settings SET can_register = $1")
        .bind(enabled)
        .execute(pool)
        .await
        .expect("set can_register");
}

/// `email` is the proven address, so passing `Some` seeds a confirmed account
/// and `None` seeds one with nothing on file. Use [`set_pending_email`] on top
/// for an address still waiting on its link.
async fn seed_password_user(pool: &PgPool, id: i32, name: &str, login: &str, email: Option<&str>) {
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
    .bind(name)
    .bind(login)
    .bind(email)
    .bind(&hash)
    .execute(pool)
    .await
    .expect("seed password user");
}

async fn count_logins(pool: &PgPool, login: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE login = $1")
        .bind(login)
        .fetch_one(pool)
        .await
        .expect("count users by login")
}

/// Stand in for a `PATCH /users/me` email change, which can't run here — it
/// only writes `pending_email` after mailing a link, and there's no relay.
async fn set_pending_email(pool: &PgPool, id: i32, email: &str) {
    sqlx::query("UPDATE users SET pending_email = $1 WHERE id = $2")
        .bind(email)
        .bind(id)
        .execute(pool)
        .await
        .expect("set pending address");
}

async fn addresses(pool: &PgPool, id: i32) -> (Option<String>, Option<String>) {
    sqlx::query_as("SELECT email, pending_email FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("read addresses")
}

fn confirm_token(user_id: i32, email: &str) -> String {
    let key = EncodingKey::from_secret(common::TEST_JWT_SECRET);
    mint_confirm_token(user_id, email, &key).expect("mint confirm token")
}

/// Hand-rolled twin of the private `ConfirmClaims`, so tests can mint the
/// shapes the real minter refuses to make: an expired one, and one tagged for
/// something other than confirmation.
#[derive(Serialize)]
struct RawConfirm {
    sub: i32,
    email: String,
    kind: &'static str,
    exp: i64,
}

fn raw_token(claims: &RawConfirm) -> String {
    let key = EncodingKey::from_secret(common::TEST_JWT_SECRET);
    encode(&Header::new(Algorithm::HS256), claims, &key).expect("mint raw token")
}

/// Year 2100 — far enough out that `exp` stays valid without the test baking
/// in a "today" that goes stale.
fn far_future() -> i64 {
    4_102_444_800
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

fn location(resp: &axum::response::Response) -> String {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

fn session_cookie(resp: &axum::response::Response) -> Option<String> {
    resp.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("tengri-jwt="))
        .map(str::to_owned)
}
