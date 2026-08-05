//! The `flights` row writer. Both ingest paths write the same shape; only the
//! `flight_id` source (NanoID vs. `LEO-<n>`) and the conflict policy differ, so
//! the two entry points share [`INSERT_FLIGHT_SQL`] and the bind order below.

use sqlx::{Postgres, Transaction};

use super::sites::find_nearest_site;

/// Everything the `flights` writer needs to insert one row. Bundled so adding
/// columns to `flights` doesn't keep growing the writer signatures past
/// readability.
///
/// Coordinates are E5 micro-degrees; the SQL converts to degrees at the bind
/// site (`coord as f64 / 1e5`) and wraps in `ST_SetSRID(ST_MakePoint(lon, lat),
/// 4326)::geography`.
///
/// `(brand_id, kind, model_id)` is the composite FK to `models`. Every ingest
/// path resolves a wing first; there's no "no glider metadata" shape anymore.
/// `propulsion` and `launch_method` are bound as text and cast at the SQL layer
/// (`$N::propulsion`, `$N::launch_method`); the caller passes the enum variant
/// verbatim ("free" / "self_launch" / "powered" and "foot" / "winch" /
/// "aerotow").
pub struct FlightRow<'a> {
    pub flight_id: &'a str,
    pub user_id: i32,
    pub takeoff_at: i64,
    pub landing_at: i64,
    pub takeoff_timezone: &'a str,
    pub landing_timezone: &'a str,
    pub takeoff_lat: i32,
    pub takeoff_lon: i32,
    pub landing_lat: i32,
    pub landing_lon: i32,
    pub brand_id: &'a str,
    pub kind: &'a str,
    pub model_id: &'a str,
    pub propulsion: &'a str,
    pub launch_method: &'a str,
}

/// What can go wrong inserting into `flights`. The other two writers only
/// return `sqlx::Error` directly because their failure modes (NOT NULL on a
/// column we always populate, FK to `flights` we just wrote in the same
/// transaction) aren't worth giving named variants.
#[derive(Debug, thiserror::Error)]
pub enum InsertFlightError {
    /// FK violation: no `users` row with the given id. Surfaced separately
    /// because both ingest paths want to nudge the operator ("create the user"
    /// / "run `leonardo migrate` first") rather than print a raw SQLSTATE.
    #[error("no users row for user_id={0}")]
    MissingUser(i32),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Column list shared by both `INSERT … VALUES` paths. Kept as a constant so
/// the placeholder numbering below matches the bind order in both writers.
const INSERT_FLIGHT_SQL: &str = "INSERT INTO flights \
    (id, user_id, takeoff_at, landing_at, takeoff_timezone, landing_timezone, \
     takeoff_point, landing_point, brand_id, kind, model_id, \
     propulsion, launch_method, \
     takeoff_country, closest_takeoff_id, closest_takeoff_distance) \
    VALUES \
    ($1, $2, to_timestamp($3), to_timestamp($4), $5, $6, \
     ST_SetSRID(ST_MakePoint($7, $8), 4326)::geography, \
     ST_SetSRID(ST_MakePoint($9, $10), 4326)::geography, \
     $11, $12::glider_kind, $13, \
     $14::propulsion, $15::launch_method, \
     $16, $17, $18)";

/// Insert one `flights` row. Errors out on every conflict — use
/// [`insert_flight_idempotent`] if the caller needs to skip rows it has already
/// imported.
pub async fn insert_flight(
    tx: &mut Transaction<'_, Postgres>,
    row: &FlightRow<'_>,
) -> Result<(), InsertFlightError> {
    let near = find_nearest_site(tx, row.takeoff_lat, row.takeoff_lon)
        .await
        .map_err(InsertFlightError::Db)?;
    sqlx::query(INSERT_FLIGHT_SQL)
        .bind(row.flight_id)
        .bind(row.user_id)
        .bind(row.takeoff_at)
        .bind(row.landing_at)
        .bind(row.takeoff_timezone)
        .bind(row.landing_timezone)
        .bind(row.takeoff_lon as f64 / 1e5)
        .bind(row.takeoff_lat as f64 / 1e5)
        .bind(row.landing_lon as f64 / 1e5)
        .bind(row.landing_lat as f64 / 1e5)
        .bind(row.brand_id)
        .bind(row.kind)
        .bind(row.model_id)
        .bind(row.propulsion)
        .bind(row.launch_method)
        .bind(near.as_ref().and_then(|n| n.country.clone()))
        .bind(near.as_ref().map(|n| n.id))
        .bind(near.as_ref().map(|n| n.distance_m))
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
    let near = find_nearest_site(tx, row.takeoff_lat, row.takeoff_lon)
        .await
        .map_err(InsertFlightError::Db)?;
    let sql = format!("{INSERT_FLIGHT_SQL} ON CONFLICT (id) DO NOTHING RETURNING id");
    let inserted: Option<String> = sqlx::query_scalar(&sql)
        .bind(row.flight_id)
        .bind(row.user_id)
        .bind(row.takeoff_at)
        .bind(row.landing_at)
        .bind(row.takeoff_timezone)
        .bind(row.landing_timezone)
        .bind(row.takeoff_lon as f64 / 1e5)
        .bind(row.takeoff_lat as f64 / 1e5)
        .bind(row.landing_lon as f64 / 1e5)
        .bind(row.landing_lat as f64 / 1e5)
        .bind(row.brand_id)
        .bind(row.kind)
        .bind(row.model_id)
        .bind(row.propulsion)
        .bind(row.launch_method)
        .bind(near.as_ref().and_then(|n| n.country.clone()))
        .bind(near.as_ref().map(|n| n.id))
        .bind(near.as_ref().map(|n| n.distance_m))
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| map_flight_error(e, row.user_id))?;
    Ok(inserted.is_some())
}

/// Translate a `flights`-table sqlx error: the FK on `user_id` has a stable
/// SQLSTATE (`23503`) and is the only error the caller needs distinguished. The
/// `user_id` we pass in is the one we just tried to bind, so we surface it
/// without parsing the diagnostic message back out of Postgres.
fn map_flight_error(e: sqlx::Error, user_id: i32) -> InsertFlightError {
    if let sqlx::Error::Database(ref db) = e
        && db.code().as_deref() == Some("23503")
    {
        return InsertFlightError::MissingUser(user_id);
    }
    InsertFlightError::Db(e)
}
