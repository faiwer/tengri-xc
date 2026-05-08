//! `leonardo check-db` — validate that the configured MySQL connection
//! works and print a short, human-friendly summary so the operator
//! knows they're pointed at the right database before anything
//! destructive runs.
//!
//! The probe is intentionally narrow: a `SELECT VERSION()`, the count
//! of `leonardo_*` tables in the current schema, and row counts for the
//! two tables we care about most (`leonardo_flights`, `leonardo_pilots`).
//! Missing tables are reported as `-` rather than failing the command,
//! so this still works against a fresh / partially-imported database.

use anyhow::Context;
use sqlx::Row;

use super::shared::connect_mysql_pool;

pub async fn run() -> anyhow::Result<()> {
    let pool = connect_mysql_pool().await?;

    let version: String = sqlx::query_scalar("SELECT VERSION()")
        .fetch_one(&pool)
        .await
        .context("querying server version")?;

    let database: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await
        .context("querying current database")?;

    let leonardo_table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name LIKE 'leonardo\\_%'",
    )
    .fetch_one(&pool)
    .await
    .context("counting leonardo_* tables")?;

    let flights = optional_count(&pool, "leonardo_flights").await?;
    let pilots = optional_count(&pool, "leonardo_pilots").await?;

    println!("connected to MySQL");
    println!("  server   {version}");
    println!("  database {database}");
    println!("  tables   {leonardo_table_count} (leonardo_*)");
    println!("  flights  {}", fmt_count(flights));
    println!("  pilots   {}", fmt_count(pilots));

    Ok(())
}

/// Count rows in a table, returning `None` if the table doesn't exist.
/// We probe `information_schema` first so a missing table is a clean
/// "not present" rather than a generic SQL error — the command is also
/// a sanity check for half-imported / customised Leonardo schemas.
async fn optional_count(pool: &sqlx::MySqlPool, table: &str) -> anyhow::Result<Option<i64>> {
    let exists: bool = sqlx::query(
        "SELECT EXISTS( \
             SELECT 1 FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = ? \
         )",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .with_context(|| format!("checking for table {table}"))?
    .try_get::<i64, _>(0)?
        != 0;

    if !exists {
        return Ok(None);
    }

    let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM `{table}`"))
        .fetch_one(pool)
        .await
        .with_context(|| format!("counting rows in {table}"))?;
    Ok(Some(count))
}

fn fmt_count(count: Option<i64>) -> String {
    match count {
        Some(n) => n.to_string(),
        None => "-".to_string(),
    }
}
