//! `tengri upgrade-tracks` — re-encode every `flight_tracks` row whose
//! `version` lags behind the current build. The fresh bytes are derived
//! from the matching `flight_sources` row (we can't re-decode the stale
//! blob — the wire format changed, that's the whole reason for the
//! upgrade).

use std::io::Read;

use anyhow::{Context, anyhow};
use flate2::read::GzDecoder;
use sqlx::{PgPool, Row};
use tengri_server::flight::{Metadata, TengriFile, encode, etag_for, parse_str, tengri::VERSION};

use super::shared::connect_pool;

pub async fn run(dry_run: bool) -> anyhow::Result<()> {
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
