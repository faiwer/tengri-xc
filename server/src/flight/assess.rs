//! Part 2 of ingest: score a persisted flight's routes and store them. The
//! upload queue worker calls [`score_and_store`]; the scoring itself mirrors
//! `tengri score` — load the stored full track, slice to its flight window,
//! evaluate every route type.

use anyhow::{Context, anyhow};
use sqlx::PgPool;
use tengri_formats::{TengriFile, Track, decode, find_flight_window, slice_flight_window};

use super::{RouteEvaluation, ScoringOutcome, evaluate_routes, store};

/// [`evaluate_stored_flight`] + persist the routes in one transaction. Returns
/// the number of routes written.
pub async fn score_and_store(pool: &PgPool, flight_id: &str) -> anyhow::Result<u64> {
    let evaluation = evaluate_stored_flight(pool, flight_id).await?;
    let mut tx = pool.begin().await.context("starting route transaction")?;
    let saved = store::upsert_scored_routes(&mut tx, flight_id, &evaluation).await?;
    tx.commit().await.context("committing routes")?;
    Ok(saved)
}

/// Load the stored full track for `flight_id` and evaluate all route types over
/// its flight window. The CPU-heavy scoring runs on a blocking thread.
pub async fn evaluate_stored_flight(
    pool: &PgPool,
    flight_id: &str,
) -> anyhow::Result<RouteEvaluation> {
    let track = load_full_track(pool, flight_id).await?;
    let flight_id = flight_id.to_owned();
    tokio::task::spawn_blocking(move || evaluate_windowed(&track, &flight_id))
        .await
        .context("scoring task panicked")?
}

fn evaluate_windowed(track: &Track, flight_id: &str) -> anyhow::Result<RouteEvaluation> {
    let window = find_flight_window(track)
        .ok_or_else(|| anyhow!("no takeoff/landing detected in flight {flight_id}"))?;
    let sliced = slice_flight_window(track.clone(), window);
    match evaluate_routes(&sliced) {
        ScoringOutcome::Answer(evaluation) => Ok(evaluation),
        ScoringOutcome::NoAnswer => Err(anyhow!("scoring produced no route evaluation")),
        ScoringOutcome::Error(error) => Err(error).context("scoring failed"),
    }
}

async fn load_full_track(pool: &PgPool, flight_id: &str) -> anyhow::Result<Track> {
    let bytes: Vec<u8> = sqlx::query_scalar(
        "SELECT bytes FROM flight_tracks WHERE flight_id = $1 AND kind = 'full'",
    )
    .bind(flight_id)
    .fetch_optional(pool)
    .await
    .context("loading full track bytes")?
    .ok_or_else(|| anyhow!("no full track for flight id {flight_id}"))?;
    let envelope = TengriFile::read_http(bytes.as_slice()).context("decoding .tengri track")?;
    decode(&envelope.track).context("decoding compact track")
}
