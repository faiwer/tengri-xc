//! The `flight_tracks` row: the compact binary track the client decodes.

use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use tengri_formats::{TengriFile, Track, decode};

pub async fn insert_track(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    version: i16,
    etag: &str,
    track_bytes: &[u8],
    compression_ratio: f32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO flight_tracks (flight_id, kind, version, etag, bytes, compression_ratio) \
         VALUES ($1, 'full', $2, $3, $4, $5)",
    )
    .bind(flight_id)
    .bind(version)
    .bind(etag)
    .bind(track_bytes)
    .bind(compression_ratio)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Decode the stored blob back into points. `Ok(None)` when the flight has no
/// track row — the caller decides whether that's a 404.
pub async fn fetch_full_track(pool: &PgPool, flight_id: &str) -> anyhow::Result<Option<Track>> {
    let bytes: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT bytes FROM flight_tracks WHERE flight_id = $1 AND kind = 'full'",
    )
    .bind(flight_id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("fetching track for flight {flight_id}"))?;

    let Some(bytes) = bytes else {
        return Ok(None);
    };

    let envelope = TengriFile::read_http(bytes.as_slice()).context("decoding .tengri track")?;
    decode(&envelope.track)
        .context("decoding compact track")
        .map(Some)
}
