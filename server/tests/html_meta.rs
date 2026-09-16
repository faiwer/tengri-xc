//! The SPA shell served by the root router's fallback: per-route `<title>` and
//! preview tags, and the status code a crawler sees for an unknown flight.

mod common;

use axum::{
    Router,
    body::Body,
    http::{StatusCode, header},
    response::Html,
    routing::get,
};
use http_body_util::BodyExt;
use serial_test::serial;
use sqlx::PgPool;
use tower::ServiceExt;

/// Stands in for the static host. Mirrors the real `client/index.html` closely
/// enough to exercise the parser: a charset above the title, a title to replace.
const SHELL: &str = "<!doctype html>\n<html lang=\"en\">\n  <head>\n    \
                     <meta charset=\"UTF-8\" />\n    <title>Tengri XC</title>\n  </head>\n  \
                     <body><div id=\"root\"></div></body>\n</html>\n";

#[tokio::test]
#[serial]
async fn a_scored_flight_gets_its_own_title_and_tags() {
    let (app, pool) = common::test_root_app(spawn_shell_host().await).await;
    common::seed_user(&pool, 7, "Alexey").await;
    common::seed_flight(&pool, "abc123XY", 7).await;
    score_flight(&pool, "abc123XY").await;

    let response = app.oneshot(common::get("/flight/abc123XY")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-cache")
    );

    let body = body_string(response).await;
    assert!(body.contains("<title>Alexey - 123.4 km FAI triangle | Tengri XC</title>"));
    assert!(body.contains(
        r#"<meta property="og:title" content="Alexey - 123.4 km FAI triangle | Tengri XC" />"#
    ));
    assert!(body.contains("245.67 points."));
    // The shell's own markup survives around the injection.
    assert!(body.contains(r#"<div id="root"></div>"#));
    assert!(!body.contains("<title>Tengri XC</title>"));
}

#[tokio::test]
#[serial]
async fn an_unknown_flight_still_renders_the_shell_but_404s() {
    let (app, _pool) = common::test_root_app(spawn_shell_host().await).await;

    let response = app.oneshot(common::get("/flight/missing1")).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = body_string(response).await;
    assert!(body.contains("<title>Tengri XC</title>"));
    assert!(body.contains(r#"<div id="root"></div>"#));
}

#[tokio::test]
#[serial]
async fn other_routes_get_the_site_name_and_description() {
    let (app, pool) = common::test_root_app(spawn_shell_host().await).await;
    sqlx::query("UPDATE site_settings SET site_description = $1 WHERE id = TRUE")
        .bind("Cross-country flights in Kyrgyzstan.")
        .execute(&pool)
        .await
        .unwrap();

    let response = app.oneshot(common::get("/flights")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response).await;
    assert!(body.contains("<title>Tengri XC</title>"));
    assert!(
        body.contains(
            r#"<meta name="description" content="Cross-country flights in Kyrgyzstan." />"#
        )
    );
    assert!(body.contains(r#"<meta property="og:image" content="http://127.0.0.1"#));
}

#[tokio::test]
#[serial]
async fn a_path_with_an_extension_is_not_a_document() {
    let (app, _pool) = common::test_root_app(spawn_shell_host().await).await;

    let response = app
        .oneshot(common::get("/assets/index-abc123.js"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Serve [`SHELL`] on an ephemeral port and return its origin, ready to be
/// handed to the server as `APP_BASE_URL`.
async fn spawn_shell_host() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route("/index.html", get(|| async { Html(SHELL) }));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// 123.4 km FAI triangle for 245.67 points, as the scoring worker would leave
/// the denormalised columns.
async fn score_flight(pool: &PgPool, flight_id: &str) {
    sqlx::query(
        "UPDATE flights \
         SET main_route_type = 'fai_triangle', main_score = 245.67, main_distance = 123400 \
         WHERE id = $1",
    )
    .bind(flight_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn body_string(response: axum::response::Response<Body>) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}
