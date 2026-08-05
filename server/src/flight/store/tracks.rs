//! The `flight_tracks` row: the compact binary track the client decodes.

use sqlx::{Postgres, Transaction};

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
