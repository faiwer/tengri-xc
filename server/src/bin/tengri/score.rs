//! `tengri score` — evaluate route distances/points for a stored flight.

use std::time::Instant;

use anyhow::{Context, anyhow};
use tengri_server::flight::{RouteKind, RoutePoint, TengriFile, decode, evaluate_routes};

use super::shared::connect_pool;

pub async fn run(flight_id: String) -> anyhow::Result<()> {
    let pool = connect_pool().await?;
    let bytes: Vec<u8> = sqlx::query_scalar(
        "SELECT bytes FROM flight_tracks WHERE flight_id = $1 AND kind = 'full'",
    )
    .bind(&flight_id)
    .fetch_optional(&pool)
    .await
    .context("loading full track bytes")?
    .ok_or_else(|| anyhow!("no full track for flight id {flight_id}"))?;

    let envelope = TengriFile::read_http(bytes.as_slice()).context("decoding .tengri track")?;
    let track = decode(&envelope.track).context("decoding compact track")?;

    let started = Instant::now();
    let evaluation = evaluate_routes(&track);
    let elapsed = started.elapsed();

    println!("flight       {flight_id}");
    println!("points       {}", track.points.len());
    println!("elapsed_ms   {:.3}", elapsed.as_secs_f64() * 1000.0);
    println!();
    println!(
        "{:<22} {:>12} {:>12} {:>8}  turnpoints",
        "route", "distance_m", "points", "optimal"
    );
    for route in evaluation.routes {
        println!(
            "{:<22} {:>12} {:>12.2} {:>8}  {}",
            route.kind.label(),
            route.distance_m,
            route.points,
            route.optimal,
            format_turnpoints(&route.turnpoints)
        );
    }

    Ok(())
}

trait RouteKindLabel {
    fn label(self) -> &'static str;
}

impl RouteKindLabel for RouteKind {
    fn label(self) -> &'static str {
        match self {
            RouteKind::FreeDistance => "free distance",
            RouteKind::FreeTriangle => "free triangle",
            RouteKind::FaiTriangle => "FAI triangle",
            RouteKind::ClosedFreeTriangle => "closed free triangle",
            RouteKind::ClosedFaiTriangle => "closed FAI triangle",
        }
    }
}

fn format_turnpoints(points: &[RoutePoint]) -> String {
    if points.is_empty() {
        return "-".to_string();
    }

    points
        .iter()
        .map(|point| {
            format!(
                "#{}@{} ({:.5}, {:.5})",
                point.track_idx,
                point.time,
                point.lat as f64 / 1e5,
                point.lon as f64 / 1e5
            )
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}
