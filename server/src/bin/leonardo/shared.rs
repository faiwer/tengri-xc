//! Cross-subcommand helpers for the `leonardo` importer: env loading and
//! MySQL connection. Postgres helpers live in the existing `tengri`
//! binary's `shared.rs` and we'll start re-using them once an actual
//! import command lands; for now the importer only talks to MySQL.

use anyhow::Context;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};

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
