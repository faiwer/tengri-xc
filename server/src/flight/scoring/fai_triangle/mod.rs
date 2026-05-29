mod bounds;
mod closure;
mod constants;
mod evaluator;
mod geometry;
mod types;

use crate::flight::scoring::{Route, ScoringOutcome};
use crate::flight::types::Track;
use crate::geo::METERS_PER_KM;

use constants::{DEFAULT_MIN_SCORING_SIDE_KM, MIN_FAI_TO_FREE_DISTANCE_RATIO, MIN_SIDE};
pub use constants::{FAI_CLOSURE_CLOSED, FAI_CLOSURE_OPEN, FAI_CLOSURE_PREFILTER};
use evaluator::FaiTriangleEvaluator;
pub use types::{
    FaiTriangleClass, FaiTriangleClosureCacheStats, NodeTraceEvent, SummaryTraceEvent, TraceEvent,
    TraceEventKind,
};

/// Computes the minimum side length of a FAI triangle in comparison to the
/// flight's free-distance result that is considered meaningful for scoring.
pub fn min_scoring_side_for_free_distance(free_distance_m: u32) -> f64 {
    let min_side_km =
        (f64::from(free_distance_m) / METERS_PER_KM) * MIN_FAI_TO_FREE_DISTANCE_RATIO * MIN_SIDE;
    DEFAULT_MIN_SCORING_SIDE_KM.max(min_side_km)
}

pub fn evaluate_fai_triangle(track: &Track, class: FaiTriangleClass) -> ScoringOutcome<Route> {
    evaluate_fai_triangle_with_min_side(track, class, DEFAULT_MIN_SCORING_SIDE_KM)
}

/// min_scoring_side_km is used to ignore triangles that are too small in
/// comparison to the flight's free-distance result.
pub fn evaluate_fai_triangle_with_min_side(
    track: &Track,
    class: FaiTriangleClass,
    min_scoring_side_km: f64,
) -> ScoringOutcome<Route> {
    let mut evaluator = FaiTriangleEvaluator::new(track, class, min_scoring_side_km);
    evaluator.evaluate(None)
}

pub fn evaluate_fai_triangle_with_min_side_traced(
    track: &Track,
    class: FaiTriangleClass,
    min_scoring_side_km: f64,
    trace: &mut dyn FnMut(&TraceEvent),
) -> ScoringOutcome<Route> {
    let mut evaluator = FaiTriangleEvaluator::new(track, class, min_scoring_side_km);
    evaluator.evaluate(Some(trace))
}

/// Feasibility probe for the simplified-track prefilter stage.
///
/// Uses a relaxed closure threshold (`FAI_CLOSURE_PREFILTER`) to avoid false
/// negatives caused by RDP simplification shifting triangle closure slightly.
/// The result is a signal, not a lower bound — the strict solver may still
/// reject the candidate.
pub fn probe_fai_triangle(track: &Track) -> ScoringOutcome<Route> {
    let mut evaluator = FaiTriangleEvaluator::new_with_closure(
        track,
        FaiTriangleClass::Open,
        FAI_CLOSURE_PREFILTER,
        DEFAULT_MIN_SCORING_SIDE_KM,
    );
    evaluator.evaluate(None)
}

pub fn evaluate_fai_triangle_traced(
    track: &Track,
    trace: &mut dyn FnMut(&TraceEvent),
) -> ScoringOutcome<Route> {
    let mut evaluator = FaiTriangleEvaluator::new_with_closure(
        track,
        FaiTriangleClass::Open,
        FAI_CLOSURE_OPEN,
        DEFAULT_MIN_SCORING_SIDE_KM,
    );
    evaluator.evaluate(Some(trace))
}

#[cfg(test)]
mod tests;
