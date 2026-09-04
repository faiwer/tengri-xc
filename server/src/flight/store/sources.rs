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

pub struct SourceDownload {
    pub format: InputFormat,
    /// `YYYY-MM-DD` in the flight's takeoff timezone.
    pub takeoff_date: String,
    pub bytes: Vec<u8>,
}

/// The original upload plus the date the download filename is built from.
/// `None` when the flight has no source row.
pub async fn fetch_source_download(
    pool: &PgPool,
    flight_id: &str,
) -> anyhow::Result<Option<SourceDownload>> {
    let row = sqlx::query_as::<_, SourceDownloadRow>(
        "SELECT s.format::text AS format, s.bytes, \
                to_char(f.takeoff_at AT TIME ZONE f.takeoff_timezone, 'YYYY-MM-DD') \
                    AS takeoff_date \
         FROM flight_sources s \
         JOIN flights f ON f.id = s.flight_id \
         WHERE s.flight_id = $1",
    )
    .bind(flight_id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("fetching source for flight {flight_id}"))?;

    let Some(row) = row else {
        return Ok(None);
    };

    Ok(Some(SourceDownload {
        format: InputFormat::from_pg_enum_value(&row.format)?,
        takeoff_date: row.takeoff_date,
        bytes: gunzip_bytes(&row.bytes).context("gunzipping stored source")?,
    }))
}

#[derive(sqlx::FromRow)]
struct SourceRow {
    format: String,
    bytes: Vec<u8>,
}

#[derive(sqlx::FromRow)]
struct SourceDownloadRow {
    format: String,
    takeoff_date: String,
    bytes: Vec<u8>,
}
