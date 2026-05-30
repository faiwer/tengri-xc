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

/// Evaluate the best FAI triangle for the track.
///
/// `class`:
/// - `None` — run both `Open` and `Closed` and return whichever scores higher.
/// - `Some(c)` — run only the given class.
pub fn evaluate_fai_triangle(
    track: &Track,
    class: Option<FaiTriangleClass>,
) -> ScoringOutcome<Route> {
    evaluate_fai_triangle_with_min_side(track, class, DEFAULT_MIN_SCORING_SIDE_KM)
}

/// Like `evaluate_fai_triangle`, but with an explicit minimum scoring side.
///
/// `min_scoring_side_km` filters out triangles whose shortest leg is below the
/// given threshold, which avoids noise when comparing against a free-distance
/// result.
pub fn evaluate_fai_triangle_with_min_side(
    track: &Track,
    class: Option<FaiTriangleClass>,
    min_scoring_side_km: f64,
) -> ScoringOutcome<Route> {
    match class {
        Some(c) => {
            let mut evaluator = FaiTriangleEvaluator::new(track, c, min_scoring_side_km);
            evaluator.evaluate(None)
        }
        None => {
            let open = evaluate_fai_triangle_with_min_side(
                track,
                Some(FaiTriangleClass::Open),
                min_scoring_side_km,
            );
            let closed = evaluate_fai_triangle_with_min_side(
                track,
                Some(FaiTriangleClass::Closed),
                min_scoring_side_km,
            );
            best_outcome(open, closed)
        }
    }
}

/// Like `evaluate_fai_triangle_with_min_side`, but emits trace events for each
/// B&B node. Useful for comparing the algorithm step-by-step against the
/// Node.js reference scorer.
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

/// Return the outcome with the higher score. `Answer` beats `NoAnswer` or
/// `Error`; between two `Answer`s the higher `score` wins.
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

#[cfg(test)]
mod tests;
