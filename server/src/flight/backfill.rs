//! Re-encode every flight whose `flight_tracks` blob lags behind the current
//! [`VERSION`] and refresh the `flights` columns introduced alongside that bump
//! (offsets, geography points, …).
//!
//! Runs after `sqlx::migrate!` on every server boot (and from `tengri
//! migrate`). The "is this flight stale?" predicate is `flight_tracks.version <
//! VERSION`, so on a fresh deploy after a `VERSION` bump every existing flight
//! is picked up and re-ingested from its stored source bytes; on every
//! subsequent boot the SELECT returns no rows and the function exits
//! immediately.
//!
//! When `VERSION` bumps, this is the file that learns about the new fields:
//! extend the `UPDATE flights` statement to write whatever new column the
//! migration added. The version-based predicate stays generic.
//!
//! Sourcelessness is treated as an invariant violation, not a normal outcome:
//! every existing flight has a `flight_sources` row (verified against the live
//! DB before this code shipped), so a missing source mid-backfill points at
//! corruption that needs operator attention rather than a silent skip.

use std::io::Read;

use anyhow::{Context, anyhow};
use flate2::read::GzDecoder;
use sqlx::{PgPool, Row};

use super::{
    ingest::{InputFormat, prepare_bytes_for_storage},
    tengri::VERSION,
};

/// Find every flight whose `flight_tracks` blob lags behind the current
/// [`VERSION`], re-run the ingest pipeline against its `flight_sources` row,
/// and write the recomputed blob + sibling `flights` columns back to the DB.
/// Returns the number of rows upgraded.
pub async fn run(pool: &PgPool) -> anyhow::Result<usize> {
    let pending = fetch_pending(pool)
        .await
        .context("listing flights pending backfill")?;
    if pending.is_empty() {
        return Ok(0);
    }
    tracing::info!(n = pending.len(), version = VERSION, "backfilling flights");
    for row in &pending {
        upgrade_one(pool, row)
            .await
            .with_context(|| format!("backfilling flight {}", row.flight_id))?;
    }
    Ok(pending.len())
}

struct Pending {
    flight_id: String,
    source_format: String,
    source_bytes: Vec<u8>,
}

async fn fetch_pending(pool: &PgPool) -> anyhow::Result<Vec<Pending>> {
    let rows = sqlx::query(
        "SELECT f.id AS flight_id, s.format::text AS source_format, s.bytes AS source_bytes \
         FROM flights f \
         JOIN flight_sources s ON s.flight_id = f.id \
         JOIN flight_tracks t ON t.flight_id = f.id AND t.kind = 'full' \
         WHERE t.version < $1 \
         ORDER BY f.id",
    )
    .bind(VERSION as i16)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(Pending {
                flight_id: r.try_get::<String, _>("flight_id")?,
                source_format: r.try_get::<String, _>("source_format")?,
                source_bytes: r.try_get::<Vec<u8>, _>("source_bytes")?,
            })
        })
        .collect()
}

async fn upgrade_one(pool: &PgPool, row: &Pending) -> anyhow::Result<()> {
    let raw = gunzip(&row.source_bytes).context("gunzipping source")?;
    let format = match row.source_format.as_str() {
        "igc" => InputFormat::Igc,
        "kml" => InputFormat::Kml,
        "gpx" => InputFormat::Gpx,
        other => return Err(anyhow!("unsupported source format `{other}`")),
    };

    let prepared =
        prepare_bytes_for_storage(format, raw).map_err(|e| anyhow!("re-ingest failed: {e:#}"))?;

    // Both UPDATEs in one transaction so a flight's blob and the matching
    // `flights` columns can never get out of sync: a partial commit would leave
    // the blob at the new `VERSION` while the columns still reflect the old
    // build's view of the takeoff/landing.
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE flight_tracks \
         SET version = $1, etag = $2, bytes = $3, compression_ratio = $4 \
         WHERE flight_id = $5 AND kind = 'full'",
    )
    .bind(VERSION as i16)
    .bind(&prepared.etag)
    .bind(&prepared.track_bytes)
    .bind(prepared.compression_ratio)
    .bind(&row.flight_id)
    .execute(&mut *tx)
    .await
    .context("updating flight_tracks")?;

    sqlx::query(
        "UPDATE flights \
         SET takeoff_offset = $1, \
             landing_offset = $2, \
             takeoff_point = ST_SetSRID(ST_MakePoint($3, $4), 4326)::geography, \
             landing_point = ST_SetSRID(ST_MakePoint($5, $6), 4326)::geography \
         WHERE id = $7",
    )
    .bind(prepared.takeoff_offset)
    .bind(prepared.landing_offset)
    .bind(prepared.takeoff_lon as f64 / 1e5)
    .bind(prepared.takeoff_lat as f64 / 1e5)
    .bind(prepared.landing_lon as f64 / 1e5)
    .bind(prepared.landing_lat as f64 / 1e5)
    .bind(&row.flight_id)
    .execute(&mut *tx)
    .await
    .context("updating flights")?;

    tx.commit().await?;
    Ok(())
}

fn gunzip(gz: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    GzDecoder::new(gz).read_to_end(&mut out)?;
    Ok(out)
}
