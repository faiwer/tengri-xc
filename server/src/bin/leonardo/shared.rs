//! Cross-subcommand helpers for the `leonardo` importer: env loading,
//! MySQL connection (source), and Postgres connection (destination).

use anyhow::Context;
use sqlx::{MySqlPool, PgPool, mysql::MySqlPoolOptions, postgres::PgPoolOptions};

/// Load `server/.env` and read `LEONARDO_MYSQL_URL`.
///
/// We resolve the file path at compile time via `CARGO_MANIFEST_DIR` so
/// the binary finds its env file regardless of the current working
/// directory — same pattern the server's `main.rs` uses.
/// Missing file is not an error: production deploys inject env vars
/// directly.
pub fn leonardo_mysql_url() -> anyhow::Result<String> {
    let _ = dotenvy::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));
    std::env::var("LEONARDO_MYSQL_URL").context("LEONARDO_MYSQL_URL must be set (try server/.env)")
}

/// Open a small MySQL pool against the configured Leonardo database.
/// Two connections is plenty for a CLI; the importer is sequential and
/// we don't want to hammer a shared production server.
pub async fn connect_mysql_pool() -> anyhow::Result<MySqlPool> {
    let url = leonardo_mysql_url()?;
    MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .context("connecting to Leonardo MySQL")
}

/// Read `DATABASE_URL` (the destination Postgres) from the same
/// `server/.env` the rest of the workspace uses.
pub fn database_url() -> anyhow::Result<String> {
    let _ = dotenvy::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));
    std::env::var("DATABASE_URL").context("DATABASE_URL must be set (try server/.env)")
}

/// Open a small Postgres pool against our own database.
pub async fn connect_pg_pool() -> anyhow::Result<PgPool> {
    let url = database_url()?;
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .context("connecting to Postgres")
}

/// Spin up a single-threaded Tokio runtime on demand. The CLI is
/// otherwise fully synchronous; we don't want every subcommand paying
/// for a runtime. Mirrors the helper in the `tengri` binary so the two
/// CLIs feel the same to the user.
pub fn run_async<F>(fut: F) -> anyhow::Result<()>
where
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?
        .block_on(fut)
}
