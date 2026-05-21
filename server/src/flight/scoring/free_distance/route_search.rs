//! Dynamic-programming free-distance scorer over a shrinking candidate set.
//!
//! The rule shape is fixed: choose five ordered fixes from the track and
//! maximize the sum of the four leg distances between them. If we tried every
//! raw fix, a simple five-point search would still be too expensive on long
//! uploads, so this module first scores a reduced set of candidate indexes and
//! then repeatedly tightens that set around the best route found so far.
//!
//! ```text
//! raw track fixes:
//!   0  1  2  3  4  5  6  7  8  9  ...  n
//!      .----- dense noise -----.
//!
//! RDP candidate pass:
//!   0        3        6        9     ...  n
//!
//! DP route on candidates:
//!   P1 ------ P2 ------ P3 ------ P4 ------ P5
//!
//! refinement window around route:
//!   [near P1] xxxxx [near P2] xxxxx [near P3] xxxxx [near P4] xxxxx [near P5]
//!             ^drop           ^drop           ^drop           ^drop
//!
//! final pass:
//!   run the same exact five-point DP on raw fixes inside those windows
//! ```
//!
//! The DP table is a cache indexed by "how many legs have we already chosen?"
//! and "which candidate point is the current route end?". Each cell stores the
//! best distance found for that situation. To fill a cell, we try every earlier
//! candidate as the previous route point and keep the best one:
//!
//! ```text
//! state[leg][end] =
//!   max over start < end:
//!     state[leg - 1][start] + distance(start, end)
//! ```
//!
//! Because every transition moves from an earlier candidate to a later one, the
//! route order is guaranteed by construction.
//!
//! This makes the solve exact for the candidate set currently being considered.
//! The approximation is only in candidate selection: RDP
//! (Ramer-Douglas-Peucker) gives a coarse route shape, each refinement keeps
//! RDP points that fall inside windows around the previous winner and drops
//! everything between those windows, and the last pass expands to raw track
//! fixes near that route so the returned indexes are not limited to simplified
//! points.
//!
//! This is intentionally different from the upstream `igc-xc-score` style
//! branch-and-bound solver. Branch-and-bound can prove optimality when it
//! finishes because every unvisited range has an upper bound, but long straight
//! flights create many nearly equivalent branches and can make that proof slow.
//! Free distance is a narrow problem with a fixed five-point route, so DP gives
//! us a bounded, direct solve for each candidate set and is a better production
//! fit for this scorer.
//!
//! The trade-off is that the RDP/window candidate pruning is heuristic, but it
//! is grounded in how real flights move. The simplified track is not a
//! different flight; it is the same timestamp-ordered path with small wiggles
//! removed. If RDP keeps the path within roughly 100 m of the original shape,
//! then replacing a raw fix with its nearby simplified candidate can only move
//! that scoring point by that local margin, not by kilometres. The DP step
//! itself still does not miss a better route among the candidates it receives.
use crate::flight::types::Track;

use super::super::shared::Point;
use super::super::shared::simplify::simplify_track_to_target_count;
use super::super::{RouteKind, ScoringError};
use super::constants::{
    RDP_MAX_TOLERANCE_M, RDP_MIN_TOLERANCE_M, RDP_TARGET_POINTS, RDP_TARGET_SPREAD,
    REFINE_MIN_WINDOW_POINTS, REFINE_START_WINDOW_PERCENT,
};
use super::solver::{find_best_free_distance_dp, squeeze_route};
use super::types::{FreeDistanceScore, route_point};

pub(super) fn evaluate_dp(track: &Track) -> Result<FreeDistanceScore, ScoringError> {
    if track.points.len() < 5 {
        return Err(ScoringError::SolverFailed {
            kind: RouteKind::FreeDistance,
            reason: "track has fewer than five fixes",
        });
    }

    let indexes = find_solution(track)?;
    // We found the result. Pack and return it.
    let points = track
        .points
        .iter()
        .map(Point::from_track_point)
        .collect::<Vec<_>>();
    let distance_m = indexes
        .windows(2)
        .map(|pair| points[pair[0]].distance_haversine(&points[pair[1]]))
        .sum::<f64>();
    Ok(FreeDistanceScore {
        distance_m,
        turnpoints: indexes
            .into_iter()
            .map(|idx| route_point(idx, &track.points[idx]))
            .collect(),
    })
}

fn run_dp_algo(track: &Track, candidate_indexes: &[usize]) -> Result<Vec<usize>, ScoringError> {
    if candidate_indexes.len() < 5 {
        return Err(ScoringError::SolverFailed {
            kind: RouteKind::FreeDistance,
            reason: "candidate set has fewer than five fixes",
        });
    }

    let points = track
        .points
        .iter()
        .map(Point::from_track_point)
        .collect::<Vec<_>>();
    find_best_free_distance_dp(&points, candidate_indexes)
        .map(Vec::from)
        .ok_or(ScoringError::SolverFailed {
            kind: RouteKind::FreeDistance,
            reason: "DP found no positive-distance route",
        })
}

// Iteratively find the best solution by RDP-simplifying the track and running
// the DP-algo on the result. Return the indexes of the solution points (5 points).
fn find_solution(track: &Track) -> Result<Vec<usize>, ScoringError> {
    let mut working_track_indexes = get_initial_rdp_track_indexes(track);
    let mut fd_solution = run_dp_algo(track, &working_track_indexes)?;

    // Define how many points to keep around each found solution point in % of
    // the total track points.
    let mut window_percent = REFINE_START_WINDOW_PERCENT;
    loop {
        // Since we know the solution we assume the points that are distant from
        // the solution points are ~garbage. Filter them out.
        let compact_indexes = squeeze_route(track.points.len(), &fd_solution, window_percent);
        if compact_indexes.len() < REFINE_MIN_WINDOW_POINTS {
            // No need to RDP, it's accurate and already tiny enough to be
            // dp-ed directly in no time.
            return run_dp_algo(track, &compact_indexes);
        }

        // One more iteration. RDP the track taking into account ONLY the points
        // surrounding the current solution.
        working_track_indexes = rdp_from_indexes(track, &compact_indexes);
        // It's supposed to be the same solution as before, but more precise.
        fd_solution = run_dp_algo(track, &working_track_indexes)?;
        // Next time make the windows smaller.
        window_percent /= 2.0;
    }
}

// The 1st iteration is the longest. Find a simple RDP-track that has a good
// accuracy/speed trade-off.
fn get_initial_rdp_track_indexes(track: &Track) -> Vec<usize> {
    simplify_track_for_dp(track).unwrap_or_else(|| (0..track.points.len()).collect())
}

fn rdp_from_indexes(track: &Track, indexes: &[usize]) -> Vec<usize> {
    // […, idx1, idx2, …] -> [point1, point2, …]
    let compact_track = Track {
        start_time: track.start_time,
        points: indexes.iter().map(|&idx| track.points[idx]).collect(),
    };

    simplify_track_for_dp(&compact_track)
        .map(|candidates| candidates.into_iter().map(|idx| indexes[idx]).collect())
        .unwrap_or_else(|| indexes.to_vec())
}

fn simplify_track_for_dp(track: &Track) -> Option<Vec<usize>> {
    simplify_track_to_target_count(
        track,
        // Keep the RDP track near the target size tuned for accuracy/speed.
        (RDP_TARGET_POINTS as f64 * (1.0 - RDP_TARGET_SPREAD)) as usize,
        (RDP_TARGET_POINTS as f64 * (1.0 + RDP_TARGET_SPREAD)) as usize,
        // Bound the search so RDP does not over- or under-simplify the route.
        RDP_MIN_TOLERANCE_M,
        RDP_MAX_TOLERANCE_M,
    )
}
