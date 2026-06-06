mod constants;
mod route_search;
mod solver;
#[cfg(test)]
mod tests;
mod track;
mod types;

use crate::ScoringTrack;
use tengri_geo::METERS_PER_KM;

use super::types::leg_distance_m;
use super::{
    Route, RoutePoint, RouteSubType, RouteType, RouteWaypoint, ScoringError, ScoringOutcome,
};
use constants::FREE_DISTANCE_MULTIPLIER;
use route_search::evaluate_dp;
use track::DedupeTrack;
use types::FreeDistanceScore;

pub fn evaluate_free_distance(track: &ScoringTrack) -> ScoringOutcome<Route> {
    let scoring_track = DedupeTrack::new(track);
    let track = scoring_track.track();

    let score = match evaluate_dp(track) {
        Ok(score) => score,
        Err(ScoringError::SolverFailed {
            route_type: RouteType::FreeDistance,
            reason: "track has fewer than five fixes",
        }) => return ScoringOutcome::NoAnswer,
        Err(error) => return ScoringOutcome::Error(error),
    };
    let score = scoring_track.remap_score(score);

    ScoringOutcome::Answer(route_result(score))
}

fn route_result(score: FreeDistanceScore) -> Route {
    let turnpoints = score
        .turnpoints
        .into_iter()
        .map(RouteWaypoint::from_route_point)
        .collect::<Vec<_>>();
    let leg_distances = calc_leg_distances_m(&turnpoints);
    let distance = round_final_distance_m(leg_distances.iter().copied().sum::<u32>());
    let factor = FREE_DISTANCE_MULTIPLIER;

    Route {
        id: 0, // A stub. Will be filled in by the caller.
        flight_id: "draft".to_owned(),
        route_type: RouteType::FreeDistance,
        sub_type: RouteSubType::None,
        turnpoints,
        leg_distances,
        distance,
        closure: None,
        score: (distance as f64 / METERS_PER_KM) * factor,
        factor,
        optimal: true, // The algorithm is always optimal
        scored_ms: 0,  // A stub. Will be filled in by the caller.
    }
}

// XContest rounds recognized distance to 0.01 km.
fn round_final_distance_m(distance_m: u32) -> u32 {
    ((distance_m as f64 / 10.0).round() as u32) * 10
}

fn calc_leg_distances_m(turnpoints: &[RouteWaypoint]) -> Vec<u32> {
    turnpoints
        .windows(2)
        .map(|pair| {
            leg_distance_m(
                RoutePoint::from_waypoint(&pair[0]),
                RoutePoint::from_waypoint(&pair[1]),
            )
        })
        .collect()
}
