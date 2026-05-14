//! Postgres writers for the three-row flight bundle: `flights` +
//! `flight_sources` + `flight_tracks`.
//!
//! Every ingest path writes the same shape — only the source of the `flight_id`
//! (NanoID for interactive uploads, `LEO-<n>` for the Leonardo importer) and
//! the conflict policy (none vs. `ON CONFLICT (id) DO NOTHING`) differ. Those
//! decisions stay with the caller; this module owns the column lists, the SQL
//! strings, and the FK-violation translation, which is what diverges painfully
//! when the schema moves.
//!
//! All functions take an open `&mut Transaction` so the caller picks the
//! boundary. Callers also pick the conflict policy: the parent
//! [`insert_flight`] is non-conflicting; importers that need idempotency call
//! [`insert_flight_idempotent`] instead.
//!
//! `bigint` timestamps are unix seconds; the SQL wraps them in
//! `to_timestamp(...)` so callers don't have to think about it. The `*_lat` /
//! `*_lon` fields on [`FlightRow`] are E5 micro-degrees (matching
//! `TrackPoint`); the SQL converts to decimal degrees and wraps in
//! `ST_SetSRID(ST_MakePoint(...), 4326)::geography` at the bind site.

use sqlx::{Postgres, Transaction};

/// Everything the `flights` writer needs to insert one row. Bundled so adding
/// columns to `flights` doesn't keep growing the writer signatures past
/// readability.
///
/// Coordinates are E5 micro-degrees; the SQL converts to degrees at the bind
/// site (`coord as f64 / 1e5`) and wraps in `ST_SetSRID(ST_MakePoint(lon, lat),
/// 4326)::geography`.
///
/// `propulsion` and `launch_method` are bound as text and cast at the SQL layer
/// (`$N::propulsion`, `$N::launch_method`); the caller passes the enum variant
/// verbatim ("free" / "self_launch" / "powered" and "foot" / "winch" /
/// "aerotow"). `glider_id` references the (deduped) `gliders` row this flight
/// belongs to, or `None` when the ingest path doesn't carry glider metadata
/// (e.g. `tengri add`) — the FK column is nullable.
pub struct FlightRow<'a> {
    pub flight_id: &'a str,
    pub user_id: i32,
    pub takeoff_at: i64,
    pub landing_at: i64,
    pub takeoff_offset: i32,
    pub landing_offset: i32,
    pub takeoff_lat: i32,
    pub takeoff_lon: i32,
    pub landing_lat: i32,
    pub landing_lon: i32,
    pub glider_id: Option<i32>,
    pub propulsion: &'a str,
    pub launch_method: &'a str,
}

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

/// Column list shared by both `INSERT … VALUES` paths. Kept as a constant so
/// the placeholder numbering below matches the bind order in both writers.
const INSERT_FLIGHT_SQL: &str = "INSERT INTO flights \
    (id, user_id, takeoff_at, landing_at, takeoff_offset, landing_offset, \
     takeoff_point, landing_point, glider_id, propulsion, launch_method) \
    VALUES \
    ($1, $2, to_timestamp($3), to_timestamp($4), $5, $6, \
     ST_SetSRID(ST_MakePoint($7, $8), 4326)::geography, \
     ST_SetSRID(ST_MakePoint($9, $10), 4326)::geography, \
     $11, $12::propulsion, $13::launch_method)";

/// Insert one `flights` row. Errors out on every conflict — use
/// [`insert_flight_idempotent`] if the caller needs to skip rows it
/// has already imported.
pub async fn insert_flight(
    tx: &mut Transaction<'_, Postgres>,
    row: &FlightRow<'_>,
) -> Result<(), InsertFlightError> {
    sqlx::query(INSERT_FLIGHT_SQL)
        .bind(row.flight_id)
        .bind(row.user_id)
        .bind(row.takeoff_at)
        .bind(row.landing_at)
        .bind(row.takeoff_offset)
        .bind(row.landing_offset)
        .bind(row.takeoff_lon as f64 / 1e5)
        .bind(row.takeoff_lat as f64 / 1e5)
        .bind(row.landing_lon as f64 / 1e5)
        .bind(row.landing_lat as f64 / 1e5)
        .bind(row.glider_id)
        .bind(row.propulsion)
        .bind(row.launch_method)
        .execute(&mut **tx)
        .await
        .map_err(|e| map_flight_error(e, row.user_id))?;
    Ok(())
}

/// Insert one `flights` row with `ON CONFLICT (id) DO NOTHING`. Returns `true`
/// if the row was written, `false` if a row with that id already existed (the
/// caller should not write the children in that case — they belong to the
/// existing flight).
///
/// Uses `RETURNING id` so we can distinguish "inserted" from "already there"
/// without a separate row-count check; `fetch_optional` maps the
/// no-row-returned case to `None`.
pub async fn insert_flight_idempotent(
    tx: &mut Transaction<'_, Postgres>,
    row: &FlightRow<'_>,
) -> Result<bool, InsertFlightError> {
    let sql = format!("{INSERT_FLIGHT_SQL} ON CONFLICT (id) DO NOTHING RETURNING id");
    let inserted: Option<String> = sqlx::query_scalar(&sql)
        .bind(row.flight_id)
        .bind(row.user_id)
        .bind(row.takeoff_at)
        .bind(row.landing_at)
        .bind(row.takeoff_offset)
        .bind(row.landing_offset)
        .bind(row.takeoff_lon as f64 / 1e5)
        .bind(row.takeoff_lat as f64 / 1e5)
        .bind(row.landing_lon as f64 / 1e5)
        .bind(row.landing_lat as f64 / 1e5)
        .bind(row.glider_id)
        .bind(row.propulsion)
        .bind(row.launch_method)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| map_flight_error(e, row.user_id))?;
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
