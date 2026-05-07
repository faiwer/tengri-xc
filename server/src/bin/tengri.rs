//! `tengri` — flight-file tooling.
//!
//! Subcommands:
//! - `convert` — parse a flight log (IGC today; later GPX/KML) and write a
//!   `.tengri` envelope.
//! - `inspect` — peek inside a `.tengri` envelope without unpacking it.
//! - `add` — ingest a flight log into the database for a given user: gzipped
//!   source goes into `flight_sources`; the encoded `.tengri` HTTP wire form
//!   goes into `flight_tracks` (kind = `full`).
//! - `upgrade-tracks` — re-encode every `flight_tracks` row whose `version`
//!   lags behind the current build, sourcing the original bytes from
//!   `flight_sources`.

use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process,
};

use anyhow::{Context, anyhow};
use clap::{Parser, Subcommand};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use rand::Rng;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use tengri_server::flight::{
    Metadata, TengriFile, Track, encode, etag_for, parse_str, tengri::VERSION,
};

#[derive(Parser)]
#[command(name = "tengri", version, about = "Tengri-XC flight tooling")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Convert a flight log into a `.tengri` envelope.
    Convert {
        /// Input file (.igc).
        input: PathBuf,
        /// Output path. Defaults to `<input>.tengri`.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Inspect a `.tengri` envelope without unpacking it.
    Inspect {
        /// `.tengri` file to read.
        input: PathBuf,
    },

    /// Ingest a flight log into the database under the given user.
    /// Inserts a `flights` row, the gzipped source into `flight_sources`,
    /// and the encoded HTTP wire form into `flight_tracks` (kind = `full`).
    /// All three writes happen in a single transaction; on failure nothing
    /// is committed.
    Add {
        /// Input flight log (.igc).
        input: PathBuf,

        /// Owning user id (`users.id`). The user must already exist.
        #[arg(long = "user-id")]
        user_id: i32,
    },

    /// Re-encode every `flight_tracks` row whose `version` lags behind the
    /// current build. The fresh bytes are derived from the matching
    /// `flight_sources` row (we can't re-decode the stale blob — the wire
    /// format changed, that's the whole reason for the upgrade).
    UpgradeTracks {
        /// Print what would change without writing to the database.
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::Convert { input, output } => convert(input, output),
        Cmd::Inspect { input } => inspect(input),
        Cmd::Add { input, user_id } => run_async(add(input, user_id)),
        Cmd::UpgradeTracks { dry_run } => run_async(upgrade_tracks(dry_run)),
    }
}

/// Spin up a single-threaded Tokio runtime on demand. The CLI is otherwise
/// fully synchronous; we don't want every subcommand paying for a runtime.
fn run_async<F>(fut: F) -> anyhow::Result<()>
where
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?
        .block_on(fut)
}

fn convert(input: PathBuf, output: Option<PathBuf>) -> anyhow::Result<()> {
    let track = parse_input(&input)?;
    let n_points = track.points.len();

    let compact = encode(&track).context("encoding compact track")?;
    let envelope = TengriFile::new(Metadata::default(), compact);

    let output = output.unwrap_or_else(|| input.with_extension("tengri"));
    let f = File::create(&output).with_context(|| format!("creating {}", output.display()))?;
    envelope
        .write(BufWriter::new(f))
        .with_context(|| format!("writing {}", output.display()))?;

    let in_size = std::fs::metadata(&input)?.len();
    let out_size = std::fs::metadata(&output)?.len();
    let ratio = in_size as f64 / out_size as f64;

    println!(
        "{} → {}  ({} points, {} → {} bytes, {:.1}×)",
        input.display(),
        output.display(),
        n_points,
        in_size,
        out_size,
        ratio,
    );
    Ok(())
}

fn inspect(input: PathBuf) -> anyhow::Result<()> {
    let f = File::open(&input).with_context(|| format!("opening {}", input.display()))?;
    let envelope = TengriFile::read(BufReader::new(f))
        .with_context(|| format!("reading {}", input.display()))?;

    let body = match &envelope.track.track {
        tengri_server::flight::compact::TrackBody::Gps { fixes, coords } => {
            format!("Gps  {} fixes, {} coords", fixes.len(), coords.len())
        }
        tengri_server::flight::compact::TrackBody::Dual { fixes, coords } => {
            format!("Dual {} fixes, {} coords", fixes.len(), coords.len())
        }
    };

    println!("file        {}", input.display());
    println!("start_time  {}", envelope.track.start_time);
    println!("interval    {} s", envelope.track.interval);
    println!("body        {body}");
    println!("time_fixes  {}", envelope.track.time_fixes.len());
    Ok(())
}

async fn add(input: PathBuf, user_id: i32) -> anyhow::Result<()> {
    let format = detect_format(&input)?;
    let raw = std::fs::read(&input).with_context(|| format!("reading {}", input.display()))?;
    let track = parse_format(format, &raw)?;
    let n_points = track.points.len();

    let compact = encode(&track).context("encoding compact track")?;
    let envelope = TengriFile::new(Metadata::default(), compact);
    let track_bytes = envelope
        .to_http_bytes()
        .context("encoding TengriFile to http bytes")?;
    let etag = etag_for(&track_bytes);

    let source_gz = gzip_bytes(&raw).context("gzipping source bytes")?;

    let pool = connect_pool().await?;
    ensure_user_exists(&pool, user_id).await?;

    let flight_id = nanoid_8();
    let mut tx = pool.begin().await.context("starting transaction")?;

    sqlx::query("INSERT INTO flights (id, user_id) VALUES ($1, $2)")
        .bind(&flight_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("inserting flights row")?;

    sqlx::query(
        "INSERT INTO flight_sources (flight_id, format, bytes) \
         VALUES ($1, $2::flight_source_format, $3)",
    )
    .bind(&flight_id)
    .bind(format.db_name())
    .bind(&source_gz)
    .execute(&mut *tx)
    .await
    .context("inserting flight_sources row")?;

    sqlx::query(
        "INSERT INTO flight_tracks (flight_id, kind, version, etag, bytes) \
         VALUES ($1, 'full', $2, $3, $4)",
    )
    .bind(&flight_id)
    .bind(VERSION as i16)
    .bind(&etag)
    .bind(&track_bytes)
    .execute(&mut *tx)
    .await
    .context("inserting flight_tracks row")?;

    tx.commit().await.context("committing transaction")?;

    println!(
        "added flight {flight_id} (user {user_id}, {n_points} points, \
         source {} bytes gz, track {} bytes, etag {etag})",
        source_gz.len(),
        track_bytes.len(),
    );
    Ok(())
}

/// Recognised input format. Wraps the file-extension dispatch so the
/// matching `flight_source_format` enum value and the parser stay in
/// lockstep when we add GPX/KML.
#[derive(Debug, Clone, Copy)]
enum InputFormat {
    Igc,
}

impl InputFormat {
    fn db_name(self) -> &'static str {
        match self {
            InputFormat::Igc => "igc",
        }
    }
}

fn detect_format(input: &Path) -> anyhow::Result<InputFormat> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("igc") => Ok(InputFormat::Igc),
        Some(other) => Err(anyhow!("unsupported input format: .{other}")),
        None => Err(anyhow!(
            "input has no extension; cannot detect format: {}",
            input.display()
        )),
    }
}

fn parse_format(format: InputFormat, bytes: &[u8]) -> anyhow::Result<Track> {
    match format {
        InputFormat::Igc => {
            let raw = std::str::from_utf8(bytes).context("IGC must be UTF-8 (ASCII)")?;
            parse_str(raw).context("parsing IGC")
        }
    }
}

fn parse_input(input: &Path) -> anyhow::Result<Track> {
    let format = detect_format(input)?;
    let bytes = std::fs::read(input).with_context(|| format!("reading {}", input.display()))?;
    parse_format(format, &bytes)
}

fn gzip_bytes(raw: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(raw)?;
    Ok(gz.finish()?)
}

/// 8-char NanoID with the `[A-Za-z0-9_-]` alphabet declared in the schema
/// comment. 64 symbols × 8 chars = 48 bits of entropy, ample for the
/// expected row count and matching the spec exactly.
fn nanoid_8() -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

async fn connect_pool() -> anyhow::Result<PgPool> {
    let _ = dotenvy::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must be set (try server/.env)")?;
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .context("connecting to Postgres")
}

async fn ensure_user_exists(pool: &PgPool, user_id: i32) -> anyhow::Result<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .context("checking user exists")?;
    if !exists {
        return Err(anyhow!("no user with id={user_id}"));
    }
    Ok(())
}

async fn upgrade_tracks(dry_run: bool) -> anyhow::Result<()> {
    let pool = connect_pool().await?;

    let stale = fetch_stale_full_tracks(&pool).await?;
    println!(
        "found {} stale `full` track(s) (current version: {})",
        stale.len(),
        VERSION,
    );
    if stale.is_empty() {
        return Ok(());
    }

    let mut report = UpgradeReport::default();
    for row in stale {
        match upgrade_one(&pool, &row, dry_run).await {
            Ok(Outcome::Upgraded { from, bytes }) => {
                println!(
                    "  {} v{from}→v{VERSION}  {} bytes{}",
                    row.flight_id,
                    bytes,
                    if dry_run { "  [dry-run]" } else { "" },
                );
                report.upgraded += 1;
            }
            Ok(Outcome::SkippedNoSource) => {
                println!("  {}  skipped: no flight_sources row", row.flight_id);
                report.skipped_no_source += 1;
            }
            Ok(Outcome::SkippedFormat(fmt)) => {
                println!(
                    "  {}  skipped: source format `{fmt}` not supported yet",
                    row.flight_id
                );
                report.skipped_format += 1;
            }
            Err(err) => {
                eprintln!("  {}  error: {err:#}", row.flight_id);
                report.errors += 1;
            }
        }
    }

    println!();
    println!(
        "summary: {} upgraded, {} skipped (no source), {} skipped (format), {} errors{}",
        report.upgraded,
        report.skipped_no_source,
        report.skipped_format,
        report.errors,
        if dry_run {
            "  [dry-run, no rows written]"
        } else {
            ""
        },
    );

    if report.errors > 0 {
        Err(anyhow!("{} flight(s) failed to upgrade", report.errors))
    } else {
        Ok(())
    }
}

#[derive(Debug)]
struct StaleTrack {
    flight_id: String,
    version: i16,
}

#[derive(Default)]
struct UpgradeReport {
    upgraded: usize,
    skipped_no_source: usize,
    skipped_format: usize,
    errors: usize,
}

enum Outcome {
    Upgraded { from: i16, bytes: usize },
    SkippedNoSource,
    SkippedFormat(String),
}

async fn fetch_stale_full_tracks(pool: &PgPool) -> anyhow::Result<Vec<StaleTrack>> {
    let rows = sqlx::query(
        "SELECT flight_id, version FROM flight_tracks \
         WHERE kind = 'full' AND version < $1 \
         ORDER BY flight_id",
    )
    .bind(VERSION as i16)
    .fetch_all(pool)
    .await
    .context("querying stale flight_tracks")?;

    rows.into_iter()
        .map(|r| {
            Ok(StaleTrack {
                flight_id: r.try_get::<String, _>("flight_id")?,
                version: r.try_get::<i16, _>("version")?,
            })
        })
        .collect()
}

async fn upgrade_one(pool: &PgPool, row: &StaleTrack, dry_run: bool) -> anyhow::Result<Outcome> {
    let source: Option<(String, Vec<u8>)> =
        sqlx::query_as("SELECT format::text, bytes FROM flight_sources WHERE flight_id = $1")
            .bind(&row.flight_id)
            .fetch_optional(pool)
            .await
            .context("querying flight_sources")?;

    let Some((format, gz_bytes)) = source else {
        return Ok(Outcome::SkippedNoSource);
    };

    if format != "igc" {
        return Ok(Outcome::SkippedFormat(format));
    }

    let raw = gunzip_to_string(&gz_bytes).context("gunzipping source bytes")?;
    let track = parse_str(&raw).context("parsing IGC")?;
    let compact = encode(&track).context("encoding compact track")?;
    let envelope = TengriFile::new(Metadata::default(), compact);
    let bytes = envelope
        .to_http_bytes()
        .context("encoding TengriFile to http bytes")?;
    let etag = etag_for(&bytes);
    let n_bytes = bytes.len();

    if !dry_run {
        sqlx::query(
            "UPDATE flight_tracks \
             SET version = $1, etag = $2, bytes = $3 \
             WHERE flight_id = $4 AND kind = 'full'",
        )
        .bind(VERSION as i16)
        .bind(&etag)
        .bind(&bytes)
        .bind(&row.flight_id)
        .execute(pool)
        .await
        .context("updating flight_tracks row")?;
    }

    Ok(Outcome::Upgraded {
        from: row.version,
        bytes: n_bytes,
    })
}

fn gunzip_to_string(gz: &[u8]) -> anyhow::Result<String> {
    let mut out = String::new();
    GzDecoder::new(gz).read_to_string(&mut out)?;
    Ok(out)
}
