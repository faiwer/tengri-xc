//! Cross-subcommand utilities for the `tengri` CLI: NanoID, env loading,
//! and the Postgres pool. Format/parse/gzip helpers live in the library
//! crate as `tengri_server::flight::ingest` so the leonardo importer
//! can reuse them.

use anyhow::{Context, anyhow};
use rand::Rng;
use sqlx::{PgPool, postgres::PgPoolOptions};

pub use tengri_server::flight::ingest::parse_input;

/// 8-char NanoID with the `[A-Za-z0-9_-]` alphabet declared in the schema
/// comment. 64 symbols × 8 chars = 48 bits of entropy, ample for the
/// expected row count and matching the spec exactly.
pub fn nanoid_8() -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Load `server/.env` and read `DATABASE_URL`. Used by every subcommand
/// that talks to Postgres directly *or* shells out to a tool that needs
/// the same connection string (e.g. `tengri db` → `psql`).
pub fn database_url() -> anyhow::Result<String> {
    let _ = dotenvy::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));
    std::env::var("DATABASE_URL").context("DATABASE_URL must be set (try server/.env)")
}

pub async fn connect_pool() -> anyhow::Result<PgPool> {
    let database_url = database_url()?;
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .context("connecting to Postgres")
}

pub async fn ensure_user_exists(pool: &PgPool, user_id: i32) -> anyhow::Result<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .context("checking user exists")?;
    if !exists {
        return Err(anyhow!("no user with id={user_id}"));
    }
    Ok(())
}

/// Spin up a single-threaded Tokio runtime on demand. The CLI is otherwise
/// fully synchronous; we don't want every subcommand paying for a runtime.
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
