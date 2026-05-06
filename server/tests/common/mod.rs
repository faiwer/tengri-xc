//! Shared test harness for integration tests that need a real Postgres.
//!
//! ## How it works
//!
//! The first call to [`test_pool`] in a `cargo test` process:
//! 1. Reads `TEST_DATABASE_URL` (default
//!    `postgres://tengri:tengri@localhost:5432/tengri_test`).
//! 2. Connects with the regular `tengri` role — **no superuser required**.
//!    The DB itself must already exist; create it once per machine with:
//!    ```sh
//!    docker exec tengri-postgres createdb -U tengri tengri_test
//!    ```
//!    (Documented in `.env.example` too.)
//! 3. Drops and recreates the `public` schema, then runs `sqlx::migrate!`.
//!    This gives every `cargo test` run a known-clean schema and is cheap
//!    (~tens of ms).
//!
//! Subsequent calls within the same process reuse the same pool — the
//! schema reset only happens once per `cargo test` invocation.
//!
//! ## Test isolation
//!
//! The pool is shared across tests within a process, and `cargo test` runs
//! tests in parallel by default. To keep things sane, mark every test that
//! touches the DB with `#[serial_test::serial]`. Each test then gets
//! exclusive access and can rely on whatever fixtures it inserts via the
//! seed helpers below — without colliding with siblings.
//!
//! Tests do **not** clean up after themselves; a fresh schema appears at
//! the start of the next `cargo test` run. If a test crashes, the DB stays
//! in whatever state it was — which is useful for `psql` post-mortems.

use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

use axum::{
    Router,
    body::Body,
    http::{HeaderName, Request, header},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tengri_server::{AppState, build_app};

const DEFAULT_TEST_DB_URL: &str = "postgres://tengri:tengri@localhost:5432/tengri_test";

/// Resolves the test DB URL once and caches it. We don't want to re-read the
/// env on every test call (and risk inconsistent values mid-run).
fn test_db_url() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| {
        std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| DEFAULT_TEST_DB_URL.to_owned())
    })
}

/// Returns a pool against the test database, ready for the calling test to
/// seed its own fixtures.
///
/// On the **first** call within a `cargo test` process the schema is dropped,
/// recreated, and migrated. On every call (including the first), data from
/// every fixture table is truncated so each test sees an empty database,
/// regardless of what the previous test inserted.
///
/// A fresh `PgPool` is built per call deliberately: `#[tokio::test]` spins up
/// a new runtime per test and the pool's internal connection tasks die with
/// the previous runtime — sharing one pool across tests leads to connection
/// leaks and `PoolTimedOut`. Pools are cheap to construct.
pub async fn test_pool() -> PgPool {
    let pool = connect().await;
    if !SCHEMA_READY.swap(true, Ordering::AcqRel) {
        reset_schema(&pool).await;
    }
    truncate_data(&pool).await;
    pool
}

static SCHEMA_READY: AtomicBool = AtomicBool::new(false);

async fn connect() -> PgPool {
    let url = test_db_url();
    PgPoolOptions::new()
        .max_connections(4)
        .connect(url)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "test DB unreachable at {url}: {e}\n\
                 Hint: the test DB must exist already; create it with:\n  \
                 docker exec tengri-postgres createdb -U tengri tengri_test"
            )
        })
}

async fn reset_schema(pool: &PgPool) {
    // Wipe everything from prior runs and re-apply migrations from scratch.
    // `DROP SCHEMA public CASCADE` works as the schema owner (the `tengri`
    // role here) — no superuser needed.
    //
    // Postgres' simple `query` path uses prepared statements, which only
    // accept a single command, so we issue the two DDL statements separately.
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(pool)
        .await
        .expect("drop public schema");
    sqlx::query("CREATE SCHEMA public")
        .execute(pool)
        .await
        .expect("recreate public schema");

    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("apply migrations on test DB");
}

/// Wipe all rows but keep the schema. `RESTART IDENTITY` rewinds the user-id
/// sequence so tests can re-use deterministic ids like `1`. `CASCADE` deals
/// with the FK chain (flights → tracks/sources).
async fn truncate_data(pool: &PgPool) {
    sqlx::query(
        "TRUNCATE flight_tracks, flight_sources, flights, users \
         RESTART IDENTITY CASCADE",
    )
    .execute(pool)
    .await
    .expect("truncate fixture tables");
}

/// Convenience: build the app router on top of a pooled `AppState`.
pub async fn test_app() -> (Router, PgPool) {
    let pool = test_pool().await;
    let app = build_app(AppState::new(pool.clone()));
    (app, pool)
}

/// Insert a user with the given id/name. Returns the id.
pub async fn seed_user(pool: &PgPool, id: i32, name: &str) -> i32 {
    sqlx::query("INSERT INTO users (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .expect("seed user");
    id
}

/// Insert a flight row. Caller supplies a (NanoID-shaped) id and the owning
/// user id. Returns the flight id for chaining convenience.
pub async fn seed_flight(pool: &PgPool, flight_id: &str, user_id: i32) -> String {
    sqlx::query("INSERT INTO flights (id, user_id) VALUES ($1, $2)")
        .bind(flight_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("seed flight");
    flight_id.to_owned()
}

// --- HTTP request helpers ----------------------------------------------------
//
/// Bare GET request with an empty body.
pub fn get(uri: impl AsRef<str>) -> Request<Body> {
    Request::builder()
        .uri(uri.as_ref())
        .body(Body::empty())
        .unwrap()
}

/// GET request with a single extra header. Use for the common case of sending
/// one conditional header (e.g. `If-None-Match`).
pub fn get_with_header(
    uri: impl AsRef<str>,
    name: HeaderName,
    value: impl AsRef<str>,
) -> Request<Body> {
    Request::builder()
        .uri(uri.as_ref())
        .header(name, value.as_ref())
        .body(Body::empty())
        .unwrap()
}

/// Sugar for the conditional-GET case with `If-None-Match`.
pub fn get_if_none_match(uri: impl AsRef<str>, etag: impl AsRef<str>) -> Request<Body> {
    get_with_header(uri, header::IF_NONE_MATCH, etag)
}

// -----------------------------------------------------------------------------

/// Insert a `kind = 'full'` track row with the provided HTTP-form bytes.
/// Computes the etag from the bytes (same hash function as the production
/// write path) so tests don't have to duplicate the formula.
pub async fn seed_full_track(pool: &PgPool, flight_id: &str, bytes: Vec<u8>) -> String {
    let etag = tengri_server::flight::etag_for(&bytes);
    sqlx::query(
        "INSERT INTO flight_tracks (flight_id, kind, version, etag, bytes) \
         VALUES ($1, 'full', $2, $3, $4)",
    )
    .bind(flight_id)
    .bind(tengri_server::flight::tengri::VERSION as i16)
    .bind(&etag)
    .bind(&bytes)
    .execute(pool)
    .await
    .expect("seed full track");
    etag
}
