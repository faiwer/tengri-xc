//! `tengri add` — ingest a flight log into the database under a given user.
//!
//! Single transaction:
//! 1. `flights` — id (NanoID) and user link.
//! 2. `flight_sources` — gzipped raw upload bytes (read-side gunzips them
//!    in `upgrade-tracks` and any future re-encoder).
//! 3. `flight_tracks` — kind = `full`, `bytes` is the HTTP wire form
//!    `gzip(bincode(TengriFile))` so the route handler can stream the
//!    column straight to the client without re-compressing.

use std::path::PathBuf;

use anyhow::{Context, anyhow};
use tengri_server::flight::{
    Metadata, TengriFile, encode, etag_for, find_flight_window, tengri::VERSION,
};

use super::shared::{
    connect_pool, detect_format, ensure_user_exists, gzip_bytes, nanoid_8, normalize_for_storage,
    parse_format,
};

pub async fn run(input: PathBuf, user_id: i32) -> anyhow::Result<()> {
    let format = detect_format(&input)?;
    let raw = std::fs::read(&input).with_context(|| format!("reading {}", input.display()))?;
    let (format, raw) = normalize_for_storage(format, raw)?;
    let track = parse_format(format, &raw)?;
    let n_points = track.points.len();

    let window = find_flight_window(&track).ok_or_else(|| {
        anyhow!(
            "could not detect takeoff/landing in {} — \
             track has no flying segment",
            input.display()
        )
    })?;
    let takeoff_at = track.points[window.takeoff_idx].time as i64;
    let landed_at = track.points[window.landing_idx].time as i64;

    let compact = encode(&track).context("encoding compact track")?;
    let envelope = TengriFile::new(Metadata::default(), compact);
    let track_bytes = envelope
        .to_http_bytes()
        .context("encoding TengriFile to http bytes")?;
    let etag = etag_for(&track_bytes);

    let source_gz = gzip_bytes(&raw).context("gzipping source bytes")?;
    let compression_ratio = track_bytes.len() as f32 / source_gz.len() as f32;

    let pool = connect_pool().await?;
    ensure_user_exists(&pool, user_id).await?;

    let flight_id = nanoid_8();
    let mut tx = pool.begin().await.context("starting transaction")?;

    sqlx::query(
        "INSERT INTO flights (id, user_id, takeoff_at, landed_at) \
         VALUES ($1, $2, to_timestamp($3), to_timestamp($4))",
    )
    .bind(&flight_id)
    .bind(user_id)
    .bind(takeoff_at)
    .bind(landed_at)
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
        "INSERT INTO flight_tracks (flight_id, kind, version, etag, bytes, compression_ratio) \
         VALUES ($1, 'full', $2, $3, $4, $5)",
    )
    .bind(&flight_id)
    .bind(VERSION as i16)
    .bind(&etag)
    .bind(&track_bytes)
    .bind(compression_ratio)
    .execute(&mut *tx)
    .await
    .context("inserting flight_tracks row")?;

    tx.commit().await.context("committing transaction")?;

    let duration_min = (landed_at - takeoff_at) as f64 / 60.0;
    let compression_pct = compression_ratio * 100.0;
    println!(
        "added flight {flight_id} (user {user_id}, {n_points} points, \
         takeoff..landed = [{}..{}] / {duration_min:.1} min, \
         source {} bytes gz, track {} bytes ({compression_pct:.1}% of gz source), etag {etag})",
        window.takeoff_idx,
        window.landing_idx,
        source_gz.len(),
        track_bytes.len(),
    );
    Ok(())
}
