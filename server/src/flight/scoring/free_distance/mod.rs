mod constants;
mod route_search;
mod solver;
mod track;
mod types;

use crate::flight::types::Track;

use super::{RouteKind, RouteResult, ScoringError, ScoringOutcome};
use constants::{FREE_DISTANCE_MULTIPLIER, METERS_PER_KM};
use route_search::evaluate_dp;
use track::ScoringTrack;
use types::FreeDistanceScore;

pub fn evaluate_free_distance(track: &Track) -> ScoringOutcome<RouteResult> {
    let scoring_track = ScoringTrack::new(track);
    let track = scoring_track.track();

    let score = match evaluate_dp(track) {
        Ok(score) => score,
        Err(ScoringError::SolverFailed {
            kind: RouteKind::FreeDistance,
            reason: "track has fewer than five fixes",
        }) => return ScoringOutcome::NoAnswer,
        Err(error) => return ScoringOutcome::Error(error),
    };
    let route = route_result(RouteKind::FreeDistance, score);

    ScoringOutcome::Answer(scoring_track.remap_route(route))
}

fn route_result(kind: RouteKind, score: FreeDistanceScore) -> RouteResult {
    let distance_m = round_final_distance_m(score.distance_m);

    RouteResult {
        kind,
        distance_m,
        closure_distance_m: None, // Free distance has no closure constraint
        points: (distance_m as f64 / METERS_PER_KM) * FREE_DISTANCE_MULTIPLIER,
        turnpoints: score.turnpoints,
        optimal: true, // The algorithm is always optimal
    }
}

// XContest rounds recognized distance to 0.01 km.
fn round_final_distance_m(distance_m: f64) -> u32 {
    ((distance_m / 10.0).round() as u32) * 10.0 as u32
}
