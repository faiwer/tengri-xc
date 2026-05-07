//! `tengri` — flight-file tooling.
//!
//! Subcommands:
//! - `convert` — parse a flight log (IGC today; later GPX/KML) and write a
//!   `.tengri` envelope.
//! - `inspect` — peek inside a `.tengri` envelope without unpacking it.
//! - `upgrade-tracks` — re-encode every `flight_tracks` row whose `version`
//!   lags behind the current build, sourcing the original bytes from
//!   `flight_sources`.

use std::{
    fs::File,
    io::{BufReader, BufWriter, Read},
    path::PathBuf,
    process,
};

use anyhow::{Context, anyhow};
use clap::{Parser, Subcommand};
use flate2::read::GzDecoder;
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

fn parse_input(input: &PathBuf) -> anyhow::Result<Track> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());

    match ext.as_deref() {
        Some("igc") => {
            let raw = std::fs::read_to_string(input)
                .with_context(|| format!("reading {}", input.display()))?;
            Ok(parse_str(&raw).context("parsing IGC")?)
        }
        Some(other) => Err(anyhow!("unsupported input format: .{other}")),
        None => Err(anyhow!(
            "input has no extension; cannot detect format: {}",
            input.display()
        )),
    }
}

async fn upgrade_tracks(dry_run: bool) -> anyhow::Result<()> {
    let _ = dotenvy::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must be set (try server/.env)")?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .context("connecting to Postgres")?;

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
