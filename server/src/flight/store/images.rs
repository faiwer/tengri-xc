//! The `flight_images` row: the rendered link-preview JPEG, written the first
//! time someone asks for it and dropped when the flight is re-scored.

use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};

pub struct StoredImage {
    pub bytes: Vec<u8>,
    pub etag: String,
}

/// The ETag alone, for answering `If-None-Match` without detoasting the blob.
pub async fn fetch_image_etag(pool: &PgPool, flight_id: &str) -> anyhow::Result<Option<String>> {
    sqlx::query_scalar("SELECT etag FROM flight_images WHERE flight_id = $1")
        .bind(flight_id)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("fetching image etag for flight {flight_id}"))
}

pub async fn fetch_image(pool: &PgPool, flight_id: &str) -> anyhow::Result<Option<StoredImage>> {
    let row =
        sqlx::query_as::<_, ImageRow>("SELECT bytes, etag FROM flight_images WHERE flight_id = $1")
            .bind(flight_id)
            .fetch_optional(pool)
            .await
            .with_context(|| format!("fetching image for flight {flight_id}"))?;

    Ok(row.map(|row| StoredImage {
        bytes: row.bytes,
        etag: row.etag,
    }))
}

/// Upsert rather than insert: two processes can render the same flight at once
/// (the gate is per-process), and both writing the same picture is harmless.
pub async fn upsert_image(
    pool: &PgPool,
    flight_id: &str,
    bytes: &[u8],
    etag: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO flight_images (flight_id, bytes, etag) VALUES ($1, $2, $3) \
         ON CONFLICT (flight_id) DO UPDATE SET \
         bytes = EXCLUDED.bytes, etag = EXCLUDED.etag, created_at = now()",
    )
    .bind(flight_id)
    .bind(bytes)
    .bind(etag)
    .execute(pool)
    .await
    .with_context(|| format!("storing image for flight {flight_id}"))?;
    Ok(())
}

/// Takes the caller's transaction so the drop commits with whatever made the
/// picture stale.
pub async fn delete_image(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM flight_images WHERE flight_id = $1")
        .bind(flight_id)
        .execute(&mut **tx)
        .await
        .with_context(|| format!("deleting image for flight {flight_id}"))?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ImageRow {
    bytes: Vec<u8>,
    etag: String,
}
