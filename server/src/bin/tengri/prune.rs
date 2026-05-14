//! `tengri prune` — wipe every data table in the database while keeping the
//! schema (and the `_sqlx_migrations` ledger) intact. Roughly the moral
//! equivalent of "drop the DB and re-migrate" but cheaper and reusable for
//! tests / dev resets.
//!
//! How: a single `TRUNCATE … RESTART IDENTITY CASCADE` over every data table.
//! `RESTART IDENTITY` resets the identity sequences so a fresh dataset starts
//! from 1 again — important because the Leonardo importer bumps `users.id` past
//! `MAX(pilotID)` and we don't want a stale post-import value lingering into
//! the next session.
//!
//! Why we don't `DROP TABLE` and re-migrate: prune is meant to run in tens of
//! milliseconds and not require a separate `tengri migrate` pass afterwards. We
//! keep `_sqlx_migrations` untouched so the schema-version chain stays valid.
//!
//! Safety: prints the row counts about to be deleted and asks for `y/N`
//! confirmation. `--yes` skips the prompt for scripts/CI.

use std::io::{self, Write};

use anyhow::{Context, anyhow};
use sqlx::PgPool;

use super::shared::connect_pool;

/// Tables we wipe, listed in the order their counts are printed. The `TRUNCATE`
/// itself takes them all at once with `CASCADE`, so the order in this list is
/// purely cosmetic — but matching parent → child reads naturally in the
/// summary.
const TABLES: &[&str] = &[
    "users",
    "user_profiles",
    "brands",
    "models",
    "flights",
    "flight_tracks",
    "flight_sources",
];

pub async fn run(skip_confirm: bool) -> anyhow::Result<()> {
    let pool = connect_pool().await?;

    let counts = count_rows(&pool).await?;
    let total: i64 = counts.iter().map(|(_, n)| *n).sum();
    println!("about to truncate:");
    for (table, n) in &counts {
        println!("  {table:<16} {n} rows");
    }
    if total == 0 {
        println!("nothing to do");
        return Ok(());
    }

    if !skip_confirm && !confirm()? {
        println!("aborted");
        return Ok(());
    }

    truncate(&pool).await?;
    println!("pruned {total} rows across {} tables", counts.len());
    Ok(())
}

async fn count_rows(pool: &PgPool) -> anyhow::Result<Vec<(&'static str, i64)>> {
    let mut out = Vec::with_capacity(TABLES.len());
    for &table in TABLES {
        let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(pool)
            .await
            .with_context(|| format!("counting {table}"))?;
        out.push((table, n));
    }
    Ok(out)
}

async fn truncate(pool: &PgPool) -> anyhow::Result<()> {
    let stmt = format!("TRUNCATE {} RESTART IDENTITY CASCADE", TABLES.join(", "));
    sqlx::query(&stmt)
        .execute(pool)
        .await
        .context("truncating data tables")?;
    Ok(())
}

fn confirm() -> anyhow::Result<bool> {
    print!("type `yes` to proceed: ");
    io::stdout().flush().ok();
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|e| anyhow!("reading stdin: {e}"))?;
    Ok(line.trim().eq_ignore_ascii_case("yes"))
}
