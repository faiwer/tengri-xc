//! Cross-subcommand helpers for the `leonardo` importer: env loading,
//! MySQL connection (source), and Postgres connection (destination).

use std::path::PathBuf;

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

/// Where Leonardo's track tarball was unpacked. Read from
/// `LEONARDO_TRACKS_ROOT`; required.
pub fn tracks_root() -> anyhow::Result<PathBuf> {
    let _ = dotenvy::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));
    let root = std::env::var("LEONARDO_TRACKS_ROOT").unwrap_or_default();
    if root.is_empty() {
        anyhow::bail!("LEONARDO_TRACKS_ROOT must be set (try server/.env)");
    }
    Ok(PathBuf::from(root))
}

/// Path to one of the curated JSON inputs under `data/`. The dir is anchored at
/// compile time via `CARGO_MANIFEST_DIR` (same pattern as `.env` loading
/// above); the *contents* load at runtime so operators can edit a file and
/// re-run the migrator without rebuilding.
pub fn data_path(name: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/data")).join(name)
}

/// Read one of the `data/` JSON files. Returns `None` when the file doesn't
/// exist — every input here is "best effort" data the operator curates
/// incrementally, so a missing file is the same as "no entries this run". Other
/// read errors (permissions, I/O) bubble as `Err`.
pub fn read_data_file(name: &str) -> anyhow::Result<Option<String>> {
    let path = data_path(name);
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    }
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
