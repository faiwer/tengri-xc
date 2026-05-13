//! `tengri migrate` — apply outstanding SQL migrations from
//! `server/migrations/` to the configured database, then run any
//! Rust-side data backfills that depend on those schema changes.
//!
//! The HTTP server runs the same two-step on startup (`src/main.rs`);
//! this subcommand exists so we can apply them without spawning a
//! long-lived process — useful after manually wiping the schema, in
//! CI, or when preparing a fresh DB before the first server boot.
//! Same code path, same idempotency guarantees.

use anyhow::Context;
use tengri_server::{flight::backfill, migrate};

use super::shared::connect_pool;

pub async fn run() -> anyhow::Result<()> {
    let pool = connect_pool().await?;
    migrate::apply(&sqlx::migrate!("./migrations"), &pool).await?;
    println!("migrations applied");

    let backfilled = backfill::run(&pool).await.context("backfilling flights")?;
    if backfilled > 0 {
        println!("backfilled {backfilled} flight(s)");
    }
    Ok(())
}
