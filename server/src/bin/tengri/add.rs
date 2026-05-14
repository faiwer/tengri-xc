//! `tengri add` — ingest a flight log into the database under a given user.
//!
//! Single transaction:
//! 1. `flights` — id (NanoID), user link, and the resolved `(brand_id, kind,
//!    model_id)` triple. The wing must already exist in `models`; we don't
//!    materialise customs from this CLI (the Leonardo importer does that —
//!    pilots adding flights interactively should pick a wing they've used
//!    before, or create one in the UI first).
//! 2. `flight_sources` — gzipped raw upload bytes.
//! 3. `flight_tracks` — kind = `full`, `bytes` is the HTTP wire form
//!    `gzip(bincode(TengriFile))` so the route handler can stream the column
//!    straight to the client without re-compressing.

use std::path::PathBuf;

use anyhow::{Context, anyhow};
use sqlx::PgPool;
use tengri_server::flight::{
    ingest::prepare_path_for_storage,
    store::{FlightRow, insert_flight, insert_source, insert_track},
    tengri::VERSION,
};

use super::shared::{connect_pool, ensure_user_exists, nanoid_8};

pub async fn run(
    input: PathBuf,
    user_id: i32,
    brand: String,
    kind: String,
    model: String,
) -> anyhow::Result<()> {
    let p = prepare_path_for_storage(&input)?;
    let n_points = p.track.points.len();

    let pool = connect_pool().await?;
    ensure_user_exists(&pool, user_id).await?;
    require_model_exists(&pool, user_id, &brand, &kind, &model).await?;

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
            brand_id: &brand,
            kind: &kind,
            model_id: &model,
            propulsion: "free",
            launch_method: "foot",
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
        "added flight {flight_id} (user {user_id}, {brand}/{kind}/{model}, \
         {n_points} points, takeoff..landing = [{}..{}] / {duration_min:.1} min, \
         source {} bytes gz, track {} bytes ({compression_pct:.1}% of gz source), etag {})",
        p.window.takeoff_idx,
        p.window.landing_idx,
        p.source_gz.len(),
        p.track_bytes.len(),
        p.etag,
    );
    Ok(())
}

/// Confirm the wing exists in `models`, filtered to either canonical (`user_id
/// IS NULL`) or owned by the flight's user. Fails loudly with a useful message
/// — the FK violation alone wouldn't tell the operator whether the brand, the
/// kind, or the model was the culprit.
async fn require_model_exists(
    pool: &PgPool,
    user_id: i32,
    brand: &str,
    kind: &str,
    model: &str,
) -> anyhow::Result<()> {
    let exists: Option<bool> = sqlx::query_scalar(
        "SELECT TRUE FROM models \
         WHERE brand_id = $1 \
           AND kind     = $2::glider_kind \
           AND id       = $3 \
           AND (user_id IS NULL OR user_id = $4) \
         LIMIT 1",
    )
    .bind(brand)
    .bind(kind)
    .bind(model)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("looking up model {brand}/{kind}/{model}"))?;
    if exists.is_none() {
        return Err(anyhow!(
            "no model row for ({brand}, {kind}, {model}) visible to user {user_id} \
             — load it with `tengri import-gliders` (canonical) or create it in the UI \
             (custom) first"
        ));
    }
    Ok(())
}
