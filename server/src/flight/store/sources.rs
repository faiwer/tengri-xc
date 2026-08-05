//! The `flight_sources` row: the gzipped original upload (IGC/KML/…), written at
//! ingest and read back for re-parsing or export.

use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};

use tengri_formats::{InputFormat, Track, parse_format};

use crate::flight::ingest::gunzip_bytes;

pub async fn insert_source(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    format: &str,
    source_gz: &[u8],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO flight_sources (flight_id, format, bytes) \
         VALUES ($1, $2::flight_source_format, $3)",
    )
    .bind(flight_id)
    .bind(format)
    .bind(source_gz)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub struct StoredSource {
    pub format: InputFormat,
    pub bytes: Vec<u8>,
}

pub async fn fetch_source(pool: &PgPool, flight_id: &str) -> anyhow::Result<StoredSource> {
    let row = sqlx::query_as::<_, SourceRow>(
        "SELECT format::text AS format, bytes FROM flight_sources WHERE flight_id = $1",
    )
    .bind(flight_id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("fetching source for flight {flight_id}"))?
    .ok_or_else(|| anyhow::anyhow!("no source for flight id {flight_id}"))?;

    let format = InputFormat::from_pg_enum_value(&row.format)?;
    let bytes = gunzip_bytes(&row.bytes).context("gunzipping stored source")?;
    Ok(StoredSource { format, bytes })
}

pub async fn fetch_source_track(pool: &PgPool, flight_id: &str) -> anyhow::Result<Track> {
    let source = fetch_source(pool, flight_id).await?;
    parse_format(source.format, &source.bytes)
}

#[derive(sqlx::FromRow)]
struct SourceRow {
    format: String,
    bytes: Vec<u8>,
}
