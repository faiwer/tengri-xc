//! `tengri add` — ingest a flight log into the database under a given user.
//!
//! Single transaction:
//! 1. `flights` — id (NanoID) and user link.
//! 2. `flight_sources` — gzipped raw upload bytes (read-side gunzips them
//!    by `flight::backfill` and any future re-encoder).
//! 3. `flight_tracks` — kind = `full`, `bytes` is the HTTP wire form
//!    `gzip(bincode(TengriFile))` so the route handler can stream the
//!    column straight to the client without re-compressing.

use std::path::PathBuf;

use anyhow::Context;
use tengri_server::flight::{
    ingest::prepare_path_for_storage,
    store::{FlightRow, insert_flight, insert_source, insert_track},
    tengri::VERSION,
};

use super::shared::{connect_pool, ensure_user_exists, nanoid_8};

pub async fn run(input: PathBuf, user_id: i32) -> anyhow::Result<()> {
    let p = prepare_path_for_storage(&input)?;
    let n_points = p.track.points.len();

    let pool = connect_pool().await?;
    ensure_user_exists(&pool, user_id).await?;

    let flight_id = nanoid_8();
    let mut tx = pool.begin().await.context("starting transaction")?;

    insert_flight(
        &mut tx,
        &FlightRow {
            flight_id: &flight_id,
            user_id,
            takeoff_at: p.takeoff_at,
            landing_at: p.landing_at,
            takeoff_offset: p.takeoff_offset,
            landing_offset: p.landing_offset,
            takeoff_lat: p.takeoff_lat,
            takeoff_lon: p.takeoff_lon,
            landing_lat: p.landing_lat,
            landing_lon: p.landing_lon,
        },
    )
    .await
    .context("inserting flights row")?;
    insert_source(&mut tx, &flight_id, p.format.pg_enum_value(), &p.source_gz)
        .await
        .context("inserting flight_sources row")?;
    insert_track(
        &mut tx,
        &flight_id,
        VERSION as i16,
        &p.etag,
        &p.track_bytes,
        p.compression_ratio,
    )
    .await
    .context("inserting flight_tracks row")?;

    tx.commit().await.context("committing transaction")?;

    let duration_min = (p.landing_at - p.takeoff_at) as f64 / 60.0;
    let compression_pct = p.compression_ratio * 100.0;
    println!(
        "added flight {flight_id} (user {user_id}, {n_points} points, \
         takeoff..landing = [{}..{}] / {duration_min:.1} min, \
         source {} bytes gz, track {} bytes ({compression_pct:.1}% of gz source), etag {})",
        p.window.takeoff_idx,
        p.window.landing_idx,
        p.source_gz.len(),
        p.track_bytes.len(),
        p.etag,
    );
    Ok(())
}
