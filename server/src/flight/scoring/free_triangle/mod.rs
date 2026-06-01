mod constants;

use crate::flight::scoring::{Route, RouteSubType, RouteType, ScoringOutcome};
use crate::flight::types::Track;

use super::olc_triangle::{OlcTriangleClass, OlcTriangleEvaluator, TriangleOptions};
use constants::{
    FREE_TRIANGLE_CLOSED_MULTIPLIER, FREE_TRIANGLE_OPEN_MULTIPLIER, MIN_FREE_TO_FREE_DISTANCE_RATIO,
};

pub fn evaluate_free_triangle(track: &Track) -> ScoringOutcome<Route> {
    evaluate_free_triangle_with_floor(track, None)
}

pub fn evaluate_free_triangle_lazy(track: &Track, free_distance_m: u32) -> ScoringOutcome<Route> {
    evaluate_free_triangle_with_floor(track, Some(free_distance_m))
}

fn evaluate_free_triangle_with_floor(
    track: &Track,
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

fn evaluate_free_triangle_class(track: &Track, class: OlcTriangleClass) -> ScoringOutcome<Route> {
    let mut evaluator = OlcTriangleEvaluator::new(track, options_for_class(class));
    evaluator.evaluate(None)
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
