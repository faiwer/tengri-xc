mod bounds;
mod closure;
mod constants;
mod evaluator;
mod geometry;
mod prefilter;
mod types;

use crate::flight::scoring::{Route, ScoringOutcome};
use crate::flight::types::Track;
use crate::geo::METERS_PER_KM;

pub use constants::FAI_CLOSURE_PREFILTER;
use constants::{DEFAULT_MIN_SCORING_SIDE_KM, MIN_FAI_TO_FREE_DISTANCE_RATIO, MIN_SIDE};
use evaluator::FaiTriangleEvaluator;
pub use prefilter::{FaiTriangleLazyAudit, FaiTriangleLazySkipReason};
pub use types::{FaiTriangleClass, FaiTriangleClosureCacheStats, TraceEvent};

/// Evaluate the best FAI triangle for the track.
///
/// `class`:
/// - `None` — run `Open` first; if it answers, also run `Closed` and return
///   whichever scores higher.
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
pub(super) fn evaluate_fai_triangle_with_min_side(
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
            let ScoringOutcome::Answer(_) = open else {
                return open;
            };
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
fn evaluate_fai_triangle_with_min_side_traced(
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
pub(super) fn probe_fai_triangle(track: &Track) -> ScoringOutcome<Route> {
    let mut evaluator = FaiTriangleEvaluator::new_with_closure(
        track,
        FaiTriangleClass::Open,
        FAI_CLOSURE_PREFILTER,
        DEFAULT_MIN_SCORING_SIDE_KM,
    );
    evaluator.evaluate(None)
}

/// Evaluate the best FAI triangle for the track, but only if the prefilter
/// determines the track is a plausible candidate.
///
/// Internally checks the `Open` class first; when it answers, `Closed` is also
/// evaluated and the higher score wins. Returns `ScoringOutcome::NoAnswer`
/// immediately when the prefilter rejects the track; `audit.skip_reason` is set
/// in that case.
///
/// When `trace` is `Some`, B&B events are emitted for the `Open` class
/// (matching Node.js `igc-xc-score`). A rejected track emits no events.
pub fn evaluate_fai_triangle_lazy(
    track: &Track,
    free_distance_m: u32,
    audit: Option<&mut FaiTriangleLazyAudit>,
    trace: Option<&mut dyn FnMut(&TraceEvent)>,
) -> ScoringOutcome<Route> {
    if !prefilter::is_valuable(track, free_distance_m, audit) {
        return ScoringOutcome::NoAnswer;
    }
    let min_side_km = DEFAULT_MIN_SCORING_SIDE_KM.max(
        (f64::from(free_distance_m) / METERS_PER_KM) * MIN_FAI_TO_FREE_DISTANCE_RATIO * MIN_SIDE,
    );
    match trace {
        Some(trace) => evaluate_fai_triangle_with_min_side_traced(
            track,
            FaiTriangleClass::Open,
            min_side_km,
            trace,
        ),
        None => evaluate_fai_triangle_with_min_side(track, None, min_side_km),
    }
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
