//! `tengri delete` — remove a flight from the database.
//!
//! Single `DELETE FROM flights WHERE id = $1`; the schema's
//! `ON DELETE CASCADE` on `flight_tracks` and `flight_sources` handles
//! the rest. Strict: errors if no row matched.

use anyhow::{Context, anyhow};

use super::shared::connect_pool;

pub async fn run(flight_id: String) -> anyhow::Result<()> {
    let pool = connect_pool().await?;

    let result = sqlx::query("DELETE FROM flights WHERE id = $1")
        .bind(&flight_id)
        .execute(&pool)
        .await
        .context("deleting flights row")?;

    if result.rows_affected() == 0 {
        return Err(anyhow!("no flight with id={flight_id}"));
    }

    println!("deleted flight {flight_id}");
    Ok(())
}
