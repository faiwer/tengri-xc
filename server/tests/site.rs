//! HTTP integration tests for the `/site` (public) and `/admin/site` (admin)
//! endpoints. Covers default-values readout, the 404/200 transition on the
//! document endpoints, and the PATCH-side gating, validation, and
//! round-tripping behaviour.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use serial_test::serial;
use sqlx::Row;
use tengri_server::user::Permissions;
use tower::ServiceExt;

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ---------------------------------------------------------------------------
// GET /site (public)
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn get_site_returns_defaults_on_a_fresh_db() {
    let (app, _pool) = common::test_app().await;

    let resp = app.oneshot(common::get("/site")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_json(resp).await;
    assert_eq!(body["site_name"], "Tengri XC");
    assert_eq!(body["can_register"], true);
    // Default install: no docs published yet.
    assert_eq!(body["has_tos"], false);
    assert_eq!(body["has_privacy"], false);
}

#[tokio::test]
#[serial]
async fn get_site_does_not_require_auth() {
    let (app, _pool) = common::test_app().await;

    // No `Cookie:` header — anonymous fetch must succeed.
    let resp = app.oneshot(common::get("/site")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// GET /site/tos and GET /site/privacy (public)
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn get_site_tos_404s_when_not_published() {
    let (app, _pool) = common::test_app().await;

    let resp = app.oneshot(common::get("/site/tos")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial]
async fn get_site_privacy_404s_when_not_published() {
    let (app, _pool) = common::test_app().await;

    let resp = app.oneshot(common::get("/site/privacy")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial]
async fn get_site_tos_returns_markdown_when_published() {
    let (app, pool) = common::test_app().await;
    sqlx::query("UPDATE site_settings SET tos_md = $1 WHERE id = TRUE")
        .bind("# Terms\n\nBe nice.")
        .execute(&pool)
        .await
        .unwrap();

    let resp = app.oneshot(common::get("/site/tos")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_json(resp).await;
    assert_eq!(body["md"], "# Terms\n\nBe nice.");
}

#[tokio::test]
#[serial]
async fn get_site_reflects_publication_state() {
    // Publishing a doc must flip `has_tos` on the slim `/site` payload
    // so the FE footer can show the link without a separate probe.
    let (app, pool) = common::test_app().await;
    sqlx::query("UPDATE site_settings SET tos_md = 'x' WHERE id = TRUE")
        .execute(&pool)
        .await
        .unwrap();

    let resp = app.oneshot(common::get("/site")).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["has_tos"], true);
    assert_eq!(body["has_privacy"], false);
}

// ---------------------------------------------------------------------------
// GET /admin/site (admin)
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn get_admin_site_without_session_returns_401() {
    let (app, _pool) = common::test_app().await;

    let resp = app.oneshot(common::get("/admin/site")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn get_admin_site_without_permission_returns_403() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    // Plain logged-in user; no MANAGE_SETTINGS bit.
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/admin/site")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn get_admin_site_returns_full_state_for_admin() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Operator").await;
    let cookie = common::auth_cookie_with_permissions(
        1,
        "Operator",
        Permissions::CAN_AUTHORIZE | Permissions::MANAGE_SETTINGS,
    );

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/admin/site")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_json(resp).await;
    assert_eq!(body["site_name"], "Tengri XC");
    assert_eq!(body["can_register"], true);
    // Admin view includes the raw markdown columns; both null on
    // a fresh install.
    assert!(body["tos_md"].is_null());
    assert!(body["privacy_md"].is_null());
}

// ---------------------------------------------------------------------------
// PATCH /admin/site
// ---------------------------------------------------------------------------

fn admin_cookie() -> String {
    common::auth_cookie_with_permissions(
        1,
        "Operator",
        Permissions::CAN_AUTHORIZE | Permissions::MANAGE_SETTINGS,
    )
}

#[tokio::test]
#[serial]
async fn patch_admin_site_writes_short_fields_and_returns_full_state() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Operator").await;
    let cookie = admin_cookie();

    let resp = app
        .clone()
        .oneshot(common::json_patch_with_cookie(
            "/admin/site",
            json!({ "site_name": "Test Site", "can_register": false }),
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_json(resp).await;
    assert_eq!(body["site_name"], "Test Site");
    assert_eq!(body["can_register"], false);
    // DB actually has the values (catches a missed commit).
    let row = sqlx::query("SELECT site_name, can_register FROM site_settings WHERE id = TRUE")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.try_get::<String, _>("site_name").unwrap(), "Test Site");
    assert!(!row.try_get::<bool, _>("can_register").unwrap());
}

#[tokio::test]
#[serial]
async fn patch_admin_site_writes_and_clears_documents() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Operator").await;
    let cookie = admin_cookie();

    // Publish ToS.
    let resp = app
        .clone()
        .oneshot(common::json_patch_with_cookie(
            "/admin/site",
            json!({ "tos_md": "# Terms\n" }),
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["tos_md"], "# Terms\n");

    // `GET /site/tos` now serves it.
    let resp = app.clone().oneshot(common::get("/site/tos")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Clear via explicit null.
    let resp = app
        .clone()
        .oneshot(common::json_patch_with_cookie(
            "/admin/site",
            json!({ "tos_md": null }),
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert!(body["tos_md"].is_null());

    // And `/site/tos` 404s again.
    let resp = app.oneshot(common::get("/site/tos")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial]
async fn patch_admin_site_treats_empty_string_doc_as_clear() {
    // The form submits an empty textarea as `""`; that should behave
    // identically to an explicit `null` (clear the column), not as "set the
    // column to an empty string and have `has_tos` report true for a zero-byte
    // doc".
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Operator").await;
    sqlx::query("UPDATE site_settings SET tos_md = '# Old' WHERE id = TRUE")
        .execute(&pool)
        .await
        .unwrap();
    let cookie = admin_cookie();

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/admin/site",
            json!({ "tos_md": "" }),
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert!(body["tos_md"].is_null());
}

#[tokio::test]
#[serial]
async fn patch_admin_site_rejects_empty_site_name() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Operator").await;
    let cookie = admin_cookie();

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/admin/site",
            json!({ "site_name": "   " }),
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_json(resp).await;
    assert_eq!(body["error"], "validation");
    assert!(body["fields"]["site_name"].is_string());
}

#[tokio::test]
#[serial]
async fn patch_admin_site_rejects_oversized_document() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Operator").await;
    let cookie = admin_cookie();

    // 64 KiB + 1 = past the cap.
    let huge = "x".repeat(64 * 1024 + 1);
    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/admin/site",
            json!({ "tos_md": huge }),
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_json(resp).await;
    assert!(body["fields"]["tos_md"].is_string());
}

#[tokio::test]
#[serial]
async fn patch_admin_site_without_permission_returns_403() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Pilot").await;
    let cookie = common::auth_cookie(1, "Pilot");

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/admin/site",
            json!({ "site_name": "Snubbed" }),
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn patch_admin_site_empty_body_returns_400() {
    let (app, pool) = common::test_app().await;
    common::seed_user(&pool, 1, "Operator").await;
    let cookie = admin_cookie();

    let resp = app
        .oneshot(common::json_patch_with_cookie(
            "/admin/site",
            json!({}),
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
