//! `leonardo_flights` → `flights` + `flight_sources` + `flight_tracks`.
//!
//! Per row:
//! 1. Resolve the on-disk track path: `<tracks-root>/<yr>/<userID>/<filename>`
//!    where `yr = YEAR(DATE)` if non-zero, else literal `"0000"`. The
//!    root comes from the required `LEONARDO_TRACKS_ROOT` env var.
//! 2. Detect format, parse to a `Track`, encode the compact wire form,
//!    derive `(takeoff_at, landing_at)` via [`find_flight_window`].
//! 3. Insert all three rows in a transaction. The `flight_id` is
//!    deterministic — `LEO-<leonardo ID>` — so re-runs are idempotent
//!    via `ON CONFLICT (id) DO NOTHING` on `flights`.
//!
//! The transaction is per-row, not per-run: a single bad track
//! (missing file, parse error, no flying segment) lands in
//! [`Report::failures`] and the loop keeps going. Idempotency is
//! preserved by the conflict handler, so a half-run can be finished
//! by re-running.

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use sqlx::{MySqlPool, PgPool};
use tengri_server::flight::{
    ingest::{PrepareError, Prepared, prepare_path_for_storage},
    store::{FlightRow, InsertFlightError, insert_flight_idempotent, insert_source, insert_track},
    tengri::VERSION,
};

use super::super::shared::tracks_root;
use super::gliders::{
    GliderInput, GliderResolver, ResolveNote, Resolved, SkipReason, launch_method_for,
};
use super::progress::Progress;
use super::{Failure, Report};

pub async fn run(mysql: &MySqlPool, pg: &PgPool) -> anyhow::Result<Report> {
    let root = tracks_root()?;
    let flights = fetch(mysql).await?;
    let mut report = Report {
        table: "flights",
        inserted: 0,
        skipped: 0,
        failures: Vec::new(),
        notes: Vec::new(),
    };
    report
        .notes
        .push(format!("source flights scanned: {}", flights.len()));

    let mut resolver = GliderResolver::build(pg)
        .await
        .context("building glider resolver")?;
    let mut tally = Tally::default();
    let mut progress = Progress::new("flights", flights.len());
    for src in &flights {
        let path = expected_path(&root, src);
        let outcome = if path.exists() {
            process(pg, &mut resolver, src, &path).await
        } else {
            Err(ProcessError::MissingFile)
        };
        record(&mut report, &mut tally, src, &path, outcome);
        progress.tick();
    }
    progress.finish();
    tally.write_notes(&mut report);

    Ok(report)
}

#[derive(Default)]
struct Tally {
    missing: usize,
    broken: usize,
    no_window: usize,
    /// Brand resolved canonically, model didn't — the importer created a
    /// per-pilot custom `models` row to host the flight. Operator action:
    /// extend `*.aliases.json` or `<kind>.json` if the raw is a real model
    /// we just don't have yet, so the next run resolves it canonically.
    customs_created: Vec<String>,
}

impl Tally {
    fn write_notes(&self, report: &mut Report) {
        if self.missing > 0 {
            report
                .notes
                .push(format!("missing on disk: {}", self.missing));
        }

        if self.broken > 0 {
            report.notes.push(format!("parse errors: {}", self.broken));
        }

        if self.no_window > 0 {
            report
                .notes
                .push(format!("no takeoff/landing detected: {}", self.no_window));
        }

        if !self.customs_created.is_empty() {
            report.notes.push(format!(
                "custom models created: {} distinct wings",
                self.customs_created.len()
            ));
            for line in &self.customs_created {
                report.notes.push(format!("  {line}"));
            }
        }
    }
}

/// Translate a single row's outcome into [`Report`] mutations: bump
/// the right counter, push a failure with a stable `key` shape so
/// the summary stays grep-able, and tally the rollup categories.
/// Splitting this out keeps the per-row loop body to one line.
fn record(
    report: &mut Report,
    tally: &mut Tally,
    src: &SourceFlight,
    path: &std::path::Path,
    outcome: Result<Inserted, ProcessError>,
) {
    let key_with_path = || format!("ID={} path={}", src.id, path.display());
    match outcome {
        Ok(Inserted::New(note)) => {
            report.inserted += 1;
            tally_note(tally, src, note);
        }
        Ok(Inserted::AlreadyPresent) => report.skipped += 1,
        Err(ProcessError::MissingFile) => {
            tally.missing += 1;
            report.failures.push(Failure {
                key: format!("ID={} userID={}", src.id, src.user_id),
                reason: format!("track file not found: {}", path.display()),
            });
        }
        Err(ProcessError::Parse(e)) => {
            tally.broken += 1;
            report.failures.push(Failure {
                key: key_with_path(),
                reason: format!("parse: {e:#}"),
            });
        }
        Err(ProcessError::NoWindow) => {
            tally.no_window += 1;
            report.failures.push(Failure {
                key: key_with_path(),
                reason: "no takeoff/landing detected".to_string(),
            });
        }
        Err(ProcessError::MissingUser(uid)) => {
            report.failures.push(Failure {
                key: format!("ID={} userID={uid}", src.id),
                reason: format!("no users row for userID={uid} (run leonardo migrate first)"),
            });
        }
        Err(ProcessError::GliderUnresolved(reason)) => {
            report.failures.push(Failure {
                key: format!(
                    "ID={} userID={} cat={} brand={} glider={:?}",
                    src.id, src.user_id, src.cat, src.glider_brand_id, src.glider
                ),
                reason: format!("glider unresolved: {reason}"),
            });
        }
        Err(ProcessError::Db(e)) => {
            report.failures.push(Failure {
                key: key_with_path(),
                reason: format!("db: {e:#}"),
            });
        }
        Err(ProcessError::Other(e)) => {
            report.failures.push(Failure {
                key: key_with_path(),
                reason: format!("{e:#}"),
            });
        }
    }
}

fn tally_note(tally: &mut Tally, src: &SourceFlight, note: Option<ResolveNote>) {
    let Some(note) = note else { return };
    match note {
        ResolveNote::CustomModelCreated {
            brand,
            raw,
            kind,
            class,
        } => tally.customs_created.push(format!(
            "ID={} {brand} ({kind} / {class}) / '{raw}'",
            src.id
        )),
    }
}

#[derive(sqlx::FromRow)]
struct SourceFlight {
    id: u64,
    /// `userID` is `mediumint unsigned` in Leonardo, comfortably inside
    /// `i64`. We carry it as `i64` so the existing `format!("…userID={}…")`
    /// reporting and the `i32::try_from(...)` step downstream stay
    /// straightforward. `try_from = "u32"` does the signed/widening hop
    /// at decode time, matching the wire type sqlx returns.
    #[sqlx(try_from = "u32")]
    user_id: i64,
    /// `YYYY` if the date is real, `"0000"` for the rows whose
    /// `DATE='0000-00-00'` placeholder we still want to try resolving.
    /// Computed in MySQL so we don't have to pull DATEs that sqlx
    /// would refuse to decode under default settings.
    year_dir: String,
    filename: String,
    /// Airframe + propulsion bitmask. Split into `kind` (airframe bit;
    /// paramotor falls back to PG; everything else → `other` and the row skips)
    /// and `flights.propulsion` (engine bits: 16=paramotor, 64=powered) inside
    /// the resolver. Stored as `smallint unsigned` in Leo (`u16` over the
    /// wire); we widen to `u32` for arithmetic.
    #[sqlx(try_from = "u16")]
    cat: u32,
    /// Pilot's brand pick from Leo's `gliderBrandID` enum. `0` means the pilot
    /// didn't pick anything; the resolver skips those. Wire type is `smallint
    /// unsigned` so we decode as `u16` and widen.
    #[sqlx(try_from = "u16")]
    glider_brand_id: i32,
    /// Free-form model string the pilot typed. `varchar(50)` NOT NULL.
    glider: String,
    /// EN/DHV/AFNOR cert bitmask, populated only for PG and only when pilots
    /// filled it in. Drives the class fallback when the model arm misses
    /// (`gliderCertCategory & 64 → en_b`, etc).
    #[sqlx(try_from = "u16")]
    glider_cert_category: u32,
    /// In-class subtype, polymorphic on `cat`. Mostly noise (Leo lets pilots
    /// re-pick per flight), but `category=3` on a PG flight implies tandem on
    /// the rare model-unresolved case.
    #[sqlx(try_from = "u16")]
    category: u32,
    /// Launch method bitmask: `1=foot, 2=winch, 8=aerotow`. Nullable
    /// in MySQL (`tinyint unsigned`); `None` defaults to `foot`.
    start_type: Option<u8>,
}

async fn fetch(mysql: &MySqlPool) -> anyhow::Result<Vec<SourceFlight>> {
    sqlx::query_as::<_, SourceFlight>(
        "SELECT \
             ID                    AS id, \
             userID                AS user_id, \
             IF(YEAR(DATE)=0, '0000', DATE_FORMAT(DATE, '%Y')) AS year_dir, \
             filename, \
             cat                   AS cat, \
             gliderBrandID         AS glider_brand_id, \
             glider                AS glider, \
             gliderCertCategory    AS glider_cert_category, \
             category              AS category, \
             startType             AS start_type \
         FROM leonardo_flights \
         WHERE serverID = 0 \
           AND userID > 0 \
           AND filename <> '' \
         ORDER BY ID",
    )
    .fetch_all(mysql)
    .await
    .context("querying leonardo_flights")
}

fn expected_path(root: &Path, src: &SourceFlight) -> PathBuf {
    root.join(&src.year_dir)
        .join(src.user_id.to_string())
        .join(&src.filename)
}

enum Inserted {
    New(Option<ResolveNote>),
    AlreadyPresent,
}

enum ProcessError {
    MissingFile,
    Parse(anyhow::Error),
    NoWindow,
    MissingUser(i64),
    GliderUnresolved(SkipReason),
    Db(sqlx::Error),
    Other(anyhow::Error),
}

impl From<sqlx::Error> for ProcessError {
    fn from(e: sqlx::Error) -> Self {
        ProcessError::Db(e)
    }
}

impl From<InsertFlightError> for ProcessError {
    fn from(e: InsertFlightError) -> Self {
        // Widen the lib's `i32` user id to `i64` to match the rest
        // of this module's reporting (Leonardo's `userID` originates
        // as `mediumint unsigned` and we carry it as `i64`
        // throughout to keep the format strings consistent).
        match e {
            InsertFlightError::MissingUser(uid) => ProcessError::MissingUser(uid as i64),
            InsertFlightError::Db(e) => ProcessError::Db(e),
        }
    }
}

impl From<PrepareError> for ProcessError {
    fn from(e: PrepareError) -> Self {
        // The lib's `Encode`/`Io` variants are both "should be
        // impossible / not the row's fault" — collapse them into
        // `Other`. `Parse` and `NoWindow` map straight across because
        // they're the per-row failure shapes the operator actually
        // categorises in the summary.
        match e {
            PrepareError::Parse(e) => ProcessError::Parse(e),
            PrepareError::NoWindow => ProcessError::NoWindow,
            PrepareError::Encode(e) | PrepareError::Io(e) => ProcessError::Other(e),
        }
    }
}

async fn is_already_present(pg: &PgPool, flight_id: &str) -> Result<bool, sqlx::Error> {
    let row: Option<String> = sqlx::query_scalar("SELECT id FROM flights WHERE id = $1")
        .bind(flight_id)
        .fetch_optional(pg)
        .await?;
    Ok(row.is_some())
}

async fn process(
    pg: &PgPool,
    resolver: &mut GliderResolver,
    src: &SourceFlight,
    path: &std::path::Path,
) -> Result<Inserted, ProcessError> {
    let flight_id = format!("LEO-{}", src.id);
    if is_already_present(pg, &flight_id).await? {
        return Ok(Inserted::AlreadyPresent);
    }
    let user_id = i32::try_from(src.user_id)
        .map_err(|_| ProcessError::Other(anyhow!("userID {} doesn't fit in i32", src.user_id)))?;
    let resolved = resolver
        .resolve(
            pg,
            &GliderInput {
                leo_flight_id: src.id,
                user_id,
                cat: src.cat,
                glider_brand_id: src.glider_brand_id,
                glider_text: &src.glider,
                glider_cert_category: src.glider_cert_category,
                category: src.category,
            },
        )
        .await
        .map_err(ProcessError::GliderUnresolved)?;
    let launch_method = launch_method_for(src.start_type);
    let prepared = prepare_path_for_storage(path)?;
    insert_rows(pg, &flight_id, user_id, &prepared, resolved, launch_method).await
}

/// Write all three rows in a single transaction. Returns
/// [`Inserted::AlreadyPresent`] if the parent insert hit the conflict
/// handler (a concurrent run got there first); otherwise the children
/// follow and the transaction commits atomically.
async fn insert_rows(
    pg: &PgPool,
    flight_id: &str,
    user_id: i32,
    p: &Prepared,
    resolved: Resolved,
    launch_method: &str,
) -> Result<Inserted, ProcessError> {
    let mut tx = pg.begin().await?;

    let inserted = insert_flight_idempotent(
        &mut tx,
        &FlightRow {
            flight_id,
            user_id,
            takeoff_at: p.takeoff_at,
            landing_at: p.landing_at,
            takeoff_offset: p.takeoff_offset,
            landing_offset: p.landing_offset,
            takeoff_lat: p.takeoff_lat,
            takeoff_lon: p.takeoff_lon,
            landing_lat: p.landing_lat,
            landing_lon: p.landing_lon,
            brand_id: &resolved.brand_id,
            kind: resolved.kind,
            model_id: &resolved.model_id,
            propulsion: resolved.propulsion,
            launch_method,
        },
    )
    .await?;
    if !inserted {
        tx.rollback().await.ok();
        return Ok(Inserted::AlreadyPresent);
    }

    insert_source(&mut tx, flight_id, p.format.pg_enum_value(), &p.source_gz).await?;
    insert_track(
        &mut tx,
        flight_id,
        VERSION as i16,
        &p.etag,
        &p.track_bytes,
        p.compression_ratio,
    )
    .await?;

    tx.commit().await?;
    Ok(Inserted::New(resolved.note))
}
