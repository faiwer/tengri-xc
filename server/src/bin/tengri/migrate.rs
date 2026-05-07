//! `tengri migrate` — apply outstanding SQL migrations from
//! `server/migrations/` to the configured database.
//!
//! The HTTP server also runs migrations on startup (`src/main.rs`); this
//! subcommand exists so we can apply them without spawning a long-lived
//! process — useful after manually wiping the schema, in CI, or when
//! preparing a fresh DB before the first server boot.

use anyhow::Context;

use super::shared::connect_pool;

pub async fn run() -> anyhow::Result<()> {
    let pool = connect_pool().await?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("running migrations")?;
    println!("migrations applied");
    Ok(())
}
