mod constants;

#[cfg(test)]
mod tests;

use crate::ScoringTrack;
use crate::shared::simplify_track_to_target_count;
use crate::{Route, RouteSubType, RouteType, ScoringOutcome};

use super::olc_triangle::{OlcTriangleClass, OlcTriangleEvaluator, TriangleOptions};
use constants::{
    FREE_TRIANGLE_CLOSED_MULTIPLIER, FREE_TRIANGLE_CLOSURE_PREFILTER,
    FREE_TRIANGLE_OPEN_MULTIPLIER, MIN_COARSE_TO_FREE_DISTANCE_RATIO,
    MIN_FREE_TO_FREE_DISTANCE_RATIO, PROBE_MAX_POINTS, PROBE_MAX_TOLERANCE_M, PROBE_MIN_POINTS,
    PROBE_MIN_TOLERANCE_M,
};

pub fn evaluate_free_triangle(track: &ScoringTrack) -> ScoringOutcome<Route> {
    evaluate_free_triangle_with_floor(track, None)
}

pub fn evaluate_free_triangle_lazy(
    track: &ScoringTrack,
    free_distance_m: u32,
) -> ScoringOutcome<Route> {
    if !is_valuable(track, free_distance_m) {
        return ScoringOutcome::NoAnswer;
    }
    evaluate_free_triangle_with_floor(track, Some(free_distance_m))
}

fn evaluate_free_triangle_with_floor(
    track: &ScoringTrack,
    free_distance_m: Option<u32>,
) -> ScoringOutcome<Route> {
    let open = evaluate_free_triangle_class(track, OlcTriangleClass::Open);
    if let ScoringOutcome::Answer(route) = &open {
        if free_distance_m.is_some_and(|free_distance_m| {
            f64::from(route.distance) < f64::from(free_distance_m) * MIN_FREE_TO_FREE_DISTANCE_RATIO
        }) {
            return ScoringOutcome::NoAnswer;
        }
    } else {
        return open; // NoAsnwer or Error.
    }

    let closed = evaluate_free_triangle_class(track, OlcTriangleClass::Closed);
    best_outcome(open, closed)
}

fn evaluate_free_triangle_class(
    track: &ScoringTrack,
    class: OlcTriangleClass,
) -> ScoringOutcome<Route> {
    let mut evaluator = OlcTriangleEvaluator::new(track, options_for_class(class));
    evaluator.evaluate(None)
}

/// Since calculating the exact free triangle is expensive, we first check if
/// the track is a plausible candidate by probing a simplified track with a
/// relaxed closure threshold.
fn is_valuable(track: &ScoringTrack, free_distance_m: u32) -> bool {
    // Unlike FAI triangles, free triangles are not sensitive to the number of
    // points in the track that are lying on the same line. So we can simplify
    // it a lot, ignoring the chord points.
    let simplified = simplified_track(track);
    let coarse_free_triangle_distance_m = match probe_free_triangle(&simplified) {
        ScoringOutcome::Answer(route) => route.distance,
        _ => 0,
    };
    f64::from(coarse_free_triangle_distance_m)
        >= f64::from(free_distance_m) * MIN_COARSE_TO_FREE_DISTANCE_RATIO
}

fn probe_free_triangle(track: &ScoringTrack) -> ScoringOutcome<Route> {
    let mut evaluator = OlcTriangleEvaluator::new_with_closure(
        track,
        options_for_class(OlcTriangleClass::Open),
        FREE_TRIANGLE_CLOSURE_PREFILTER,
    );
    evaluator.evaluate(None)
}

fn simplified_track(track: &ScoringTrack) -> ScoringTrack {
    // Use binary search to find a simplified track that contains between 100
    // and 200 points. Such tracks are almost instant to score and we ensure we
    // don't over-simplify the track (<5 points = bail out)
    let indexes = simplify_track_to_target_count(
        track,
        PROBE_MIN_POINTS,
        PROBE_MAX_POINTS,
        PROBE_MIN_TOLERANCE_M,
        PROBE_MAX_TOLERANCE_M,
    )
    .unwrap_or_else(|| (0..track.points.len()).collect());
    track.select_at(indexes)
}

fn options_for_class(class: OlcTriangleClass) -> TriangleOptions {
    match class {
        OlcTriangleClass::Open => TriangleOptions {
            route_type: RouteType::FreeTriangle,
            sub_type: RouteSubType::OlcOpen,
            multiplier: FREE_TRIANGLE_OPEN_MULTIPLIER,
            min_side: None,
            min_scoring_side_km: None,
        },
        OlcTriangleClass::Closed => TriangleOptions {
            route_type: RouteType::FreeTriangle,
            sub_type: RouteSubType::OlcClosed,
            multiplier: FREE_TRIANGLE_CLOSED_MULTIPLIER,
            min_side: None,
            min_scoring_side_km: None,
        },
    }
}

fn best_outcome(a: ScoringOutcome<Route>, b: ScoringOutcome<Route>) -> ScoringOutcome<Route> {
    match (a, b) {
        (ScoringOutcome::Answer(ra), ScoringOutcome::Answer(rb)) => {
            ScoringOutcome::Answer(if ra.score >= rb.score { ra } else { rb })
        }
        (answer @ ScoringOutcome::Answer(_), _) | (_, answer @ ScoringOutcome::Answer(_)) => answer,
        (ScoringOutcome::NoAnswer, other) | (other, ScoringOutcome::NoAnswer) => other,
        (a, _) => a,
    }
}
