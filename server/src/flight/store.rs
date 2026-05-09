//! Postgres writers for the three-row flight bundle:
//! `flights` + `flight_sources` + `flight_tracks`.
//!
//! Every ingest path writes the same shape — only the source of the
//! `flight_id` (NanoID for interactive uploads, `LEO-<n>` for the
//! Leonardo importer) and the conflict policy (none vs.
//! `ON CONFLICT (id) DO NOTHING`) differ. Those decisions stay with
//! the caller; this module owns the column lists, the SQL strings,
//! and the FK-violation translation, which is what diverges painfully
//! when the schema moves.
//!
//! All functions take an open `&mut Transaction` so the caller picks
//! the boundary. Callers also pick the conflict policy: the parent
//! [`insert_flight`] is non-conflicting; importers that need
//! idempotency call [`insert_flight_idempotent`] instead.
//!
//! `bigint` timestamps are unix seconds; the SQL wraps them in
//! `to_timestamp(...)` so callers don't have to think about it.

use sqlx::{Postgres, Transaction};

/// What can go wrong inserting into `flights`. The other two writers
/// only return `sqlx::Error` directly because their failure modes
/// (NOT NULL on a column we always populate, FK to `flights` we just
/// wrote in the same transaction) aren't worth giving named variants.
#[derive(Debug, thiserror::Error)]
pub enum InsertFlightError {
    /// FK violation: no `users` row with the given id. Surfaced
    /// separately because both ingest paths want to nudge the
    /// operator ("create the user" / "run `leonardo migrate` first")
    /// rather than print a raw SQLSTATE.
    #[error("no users row for user_id={0}")]
    MissingUser(i32),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Insert one `flights` row. Errors out on every conflict — use
/// [`insert_flight_idempotent`] if the caller needs to skip rows it
/// has already imported.
pub async fn insert_flight(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    user_id: i32,
    takeoff_at: i64,
    landed_at: i64,
) -> Result<(), InsertFlightError> {
    sqlx::query(
        "INSERT INTO flights (id, user_id, takeoff_at, landed_at) \
         VALUES ($1, $2, to_timestamp($3), to_timestamp($4))",
    )
    .bind(flight_id)
    .bind(user_id)
    .bind(takeoff_at)
    .bind(landed_at)
    .execute(&mut **tx)
    .await
    .map_err(|e| map_flight_error(e, user_id))?;
    Ok(())
}

/// Insert one `flights` row with `ON CONFLICT (id) DO NOTHING`.
/// Returns `true` if the row was written, `false` if a row with that
/// id already existed (the caller should not write the children in
/// that case — they belong to the existing flight).
///
/// Implemented with `RETURNING id` because that's the cleanest "did
/// we actually insert?" signal under the conflict handler. A row
/// count check would also work but `query_scalar::<Option<_>>` is
/// the canonical sqlx idiom for this.
pub async fn insert_flight_idempotent(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    user_id: i32,
    takeoff_at: i64,
    landed_at: i64,
) -> Result<bool, InsertFlightError> {
    let inserted: Option<String> = sqlx::query_scalar(
        "INSERT INTO flights (id, user_id, takeoff_at, landed_at) \
         VALUES ($1, $2, to_timestamp($3), to_timestamp($4)) \
         ON CONFLICT (id) DO NOTHING \
         RETURNING id",
    )
    .bind(flight_id)
    .bind(user_id)
    .bind(takeoff_at)
    .bind(landed_at)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| map_flight_error(e, user_id))?;
    Ok(inserted.is_some())
}

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

/// Translate a `flights`-table sqlx error: the FK on `user_id` has a
/// stable SQLSTATE (`23503`) and is the only error the caller needs
/// distinguished. The `user_id` we pass in is the one we just tried
/// to bind, so we surface it without parsing the diagnostic message
/// back out of Postgres.
fn map_flight_error(e: sqlx::Error, user_id: i32) -> InsertFlightError {
    if let sqlx::Error::Database(ref db) = e
        && db.code().as_deref() == Some("23503")
    {
        return InsertFlightError::MissingUser(user_id);
    }
    InsertFlightError::Db(e)
}
