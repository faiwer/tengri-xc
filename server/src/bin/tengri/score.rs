//! `tengri score` — evaluate route distances/points for a stored flight.

use std::time::Instant;

use anyhow::{Context, anyhow};
use tengri_server::flight::{
    Route, RouteType, RouteWaypoint, ScoringOutcome, TengriFile, decode, evaluate_routes,
    find_flight_window, ingest::slice_flight_window, store::upsert_scored_routes,
};

use super::shared::connect_pool;

pub async fn run(flight_id: String, update_db: bool) -> anyhow::Result<()> {
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
    let window = find_flight_window(&track)
        .ok_or_else(|| anyhow!("no takeoff/landing detected in flight {flight_id}"))?;
    let track = slice_flight_window(track, window);

    let started = Instant::now();
    let evaluation = match evaluate_routes(&track) {
        ScoringOutcome::Answer(evaluation) => evaluation,
        ScoringOutcome::NoAnswer => return Err(anyhow!("scoring produced no route evaluation")),
        ScoringOutcome::Error(error) => return Err(error).context("scoring failed"),
    };
    let elapsed = started.elapsed();
    if update_db {
        let mut tx = pool
            .begin()
            .await
            .context("starting route update transaction")?;
        let saved = upsert_scored_routes(&mut tx, &flight_id, &evaluation).await?;
        tx.commit().await.context("committing route updates")?;
        println!("updated_db   {saved} routes");
    }

    println!("flight       {flight_id}");
    println!("points       {}", track.points.len());
    println!("elapsed_ms   {:.3}", elapsed.as_secs_f64() * 1000.0);
    println!();
    println!(
        "{:<22} {:>12} {:>12} {:>8}  turnpoints",
        "route", "distance_m", "score", "optimal"
    );
    for (route_type, route) in RouteType::SCORABLE.into_iter().zip(evaluation.routes) {
        print_route(route_type, route);
    }

    Ok(())
}

fn print_route(route_type: RouteType, route: ScoringOutcome<Route>) {
    match route {
        ScoringOutcome::Answer(route) => println!(
            "{:<22} {:>12} {:>12.2} {:>8}  {}",
            route.route_type.label(),
            route.distance,
            route.score,
            route.optimal,
            format_turnpoints(&route.turnpoints)
        ),
        ScoringOutcome::NoAnswer => println!(
            "{:<22} {:>12} {:>12} {:>8}  -",
            route_type.label(),
            "no answer",
            "-",
            "-"
        ),
        ScoringOutcome::Error(error) => println!(
            "{:<22} {:>12} {:>12} {:>8}  {}",
            route_type.label(),
            "error",
            "-",
            "-",
            error
        ),
    }
}

trait RouteTypeLabel {
    fn label(self) -> &'static str;
}

impl RouteTypeLabel for RouteType {
    fn label(self) -> &'static str {
        match self {
            RouteType::FreeDistance => "free distance",
            RouteType::FreeTriangle => "free triangle",
            RouteType::FaiTriangle => "FAI triangle",
            RouteType::Task => "task",
        }
    }
}

fn format_turnpoints(points: &[RouteWaypoint]) -> String {
    if points.is_empty() {
        return "-".to_string();
    }

    points
        .iter()
        .map(|point| match point {
            RouteWaypoint::Point { fix } => {
                format!(
                    "@{} ({:.5}, {:.5})",
                    fix.time,
                    fix.lat as f64 / 1e5,
                    fix.lon as f64 / 1e5
                )
            }
            // TODO: Find a better way to represent cylinder and line crossings.
            RouteWaypoint::Cylinder { track_fix, .. } | RouteWaypoint::Line { track_fix, .. } => {
                format!(
                    "@{} ({:.5}, {:.5})",
                    track_fix.time,
                    track_fix.lat as f64 / 1e5,
                    track_fix.lon as f64 / 1e5
                )
            }
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}
