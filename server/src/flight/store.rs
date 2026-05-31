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

use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};

use super::{
    Route, RouteEvaluation, RouteSubType, RouteType, ScoringOutcome, Track,
    ingest::{InputFormat, gunzip_bytes, parse_format},
};

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
    (id, user_id, takeoff_at, landing_at, takeoff_timezone, landing_timezone, \
     takeoff_point, landing_point, brand_id, kind, model_id, \
     propulsion, launch_method) \
    VALUES \
    ($1, $2, to_timestamp($3), to_timestamp($4), $5, $6, \
     ST_SetSRID(ST_MakePoint($7, $8), 4326)::geography, \
     ST_SetSRID(ST_MakePoint($9, $10), 4326)::geography, \
     $11, $12::glider_kind, $13, \
     $14::propulsion, $15::launch_method)";

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

pub async fn fetch_scored_routes(pool: &PgPool, flight_id: &str) -> anyhow::Result<Vec<Route>> {
    let rows = sqlx::query_as::<_, StoredRouteRow>(
        "SELECT flight_id, type::text AS route_type, sub_type::text AS sub_type, \
                turnpoints::text AS turnpoints, leg_distances, distance, \
                score::float8 AS score, factor::float8 AS factor, optimal, closure::text AS closure, \
                scored_ms \
         FROM routes \
         WHERE flight_id = $1",
    )
    .bind(flight_id)
    .fetch_all(pool)
    .await
    .with_context(|| format!("fetching scored routes for flight {flight_id}"))?;

    rows.into_iter().map(StoredRouteRow::into_route).collect()
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

pub async fn upsert_scored_routes(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    evaluation: &RouteEvaluation,
) -> anyhow::Result<u64> {
    let mut saved = 0;
    for outcome in &evaluation.routes {
        if let ScoringOutcome::Answer(route) = outcome {
            upsert_scored_route(tx, flight_id, route).await?;
            saved += 1;
        }
    }
    if saved > 0 {
        update_flight_main_route(tx, flight_id).await?;
    }
    Ok(saved)
}

async fn update_flight_main_route(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE flights f \
         SET main_route_id   = r.id, \
             main_route_type = r.type, \
             main_score      = r.score, \
             main_distance   = r.distance \
         FROM ( \
             SELECT id, type, score, distance \
             FROM routes \
             WHERE flight_id = $1 \
             ORDER BY score DESC \
             LIMIT 1 \
         ) r \
         WHERE f.id = $1",
    )
    .bind(flight_id)
    .execute(&mut **tx)
    .await
    .context("updating flight main route")?;
    Ok(())
}

pub async fn upsert_scored_route(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    route: &Route,
) -> anyhow::Result<()> {
    let turnpoints = serde_json::to_string(&route.turnpoints).context("serializing turnpoints")?;
    let closure = route
        .closure
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .context("serializing closure")?;
    let leg_distances = route
        .leg_distances
        .iter()
        .copied()
        .map(i32::try_from)
        .collect::<Result<Vec<_>, _>>()
        .context("converting leg distances to Postgres integer[]")?;
    let distance =
        i32::try_from(route.distance).context("converting distance to Postgres integer")?;
    let score = format!("{:.2}", route.score);
    let factor = format!("{:.1}", route.factor);

    sqlx::query(
        "INSERT INTO routes \
         (flight_id, type, sub_type, turnpoints, leg_distances, distance, score, factor, optimal, closure, scored_ms) \
         VALUES ($1, $2::route_type, $3::route_sub_type, $4::jsonb, $5, $6, $7::numeric, $8::numeric, $9, $10::jsonb, $11) \
         ON CONFLICT (flight_id, type, sub_type) DO UPDATE SET \
         turnpoints = EXCLUDED.turnpoints, \
         leg_distances = EXCLUDED.leg_distances, \
         distance = EXCLUDED.distance, \
         score = EXCLUDED.score, \
         factor = EXCLUDED.factor, \
         optimal = EXCLUDED.optimal, \
         closure = EXCLUDED.closure, \
         scored_ms = EXCLUDED.scored_ms",
    )
    .bind(flight_id)
    .bind(route_type_value(route.route_type))
    .bind(route_sub_type_value(route.sub_type))
    .bind(turnpoints)
    .bind(&leg_distances)
    .bind(distance)
    .bind(score)
    .bind(factor)
    .bind(route.optimal)
    .bind(closure)
    .bind(route.scored_ms as i32)
    .execute(&mut **tx)
    .await
    .with_context(|| {
        format!(
            "upserting {:?}/{:?} route for flight {flight_id}",
            route.route_type, route.sub_type
        )
    })?;
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

#[derive(sqlx::FromRow)]
struct StoredRouteRow {
    flight_id: String,
    route_type: String,
    sub_type: String,
    turnpoints: String,
    leg_distances: Vec<i32>,
    distance: i32,
    score: f64,
    factor: f64,
    optimal: bool,
    closure: Option<String>,
    scored_ms: i32,
}

impl StoredRouteRow {
    fn into_route(self) -> anyhow::Result<Route> {
        Ok(Route {
            flight_id: self.flight_id,
            route_type: route_type_from_value(&self.route_type)?,
            sub_type: route_sub_type_from_value(&self.sub_type)?,
            turnpoints: serde_json::from_str(&self.turnpoints)
                .context("parsing route turnpoints")?,
            leg_distances: self
                .leg_distances
                .into_iter()
                .map(u32::try_from)
                .collect::<Result<Vec<_>, _>>()
                .context("converting stored leg distances")?,
            distance: u32::try_from(self.distance).context("converting stored route distance")?,
            score: self.score,
            factor: self.factor,
            optimal: self.optimal,
            closure: self
                .closure
                .map(|closure| serde_json::from_str(&closure))
                .transpose()
                .context("parsing route closure")?,
            scored_ms: self.scored_ms as u32,
        })
    }
}

fn route_type_value(route_type: RouteType) -> &'static str {
    match route_type {
        RouteType::FreeDistance => "free_distance",
        RouteType::FaiTriangle => "fai_triangle",
        RouteType::FreeTriangle => "free_triangle",
        RouteType::Task => "task",
    }
}

fn route_type_from_value(value: &str) -> anyhow::Result<RouteType> {
    match value {
        "free_distance" => Ok(RouteType::FreeDistance),
        "fai_triangle" => Ok(RouteType::FaiTriangle),
        "free_triangle" => Ok(RouteType::FreeTriangle),
        "task" => Ok(RouteType::Task),
        _ => Err(anyhow::anyhow!("unknown route_type {value:?}")),
    }
}

fn route_sub_type_value(sub_type: RouteSubType) -> &'static str {
    match sub_type {
        RouteSubType::None => "none",
        RouteSubType::OlcClosed => "olc_closed",
        RouteSubType::OlcOpen => "olc_open",
        RouteSubType::FaiCylinders => "fai_cylinders",
    }
}

fn route_sub_type_from_value(value: &str) -> anyhow::Result<RouteSubType> {
    match value {
        "none" => Ok(RouteSubType::None),
        "olc_closed" => Ok(RouteSubType::OlcClosed),
        "olc_open" => Ok(RouteSubType::OlcOpen),
        "fai_cylinders" => Ok(RouteSubType::FaiCylinders),
        _ => Err(anyhow::anyhow!("unknown route_sub_type {value:?}")),
    }
}
