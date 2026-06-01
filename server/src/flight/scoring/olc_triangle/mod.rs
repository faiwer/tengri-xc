mod bounds;
mod closure;
mod constants;
mod evaluator;
mod geometry;
mod types;

pub use constants::FAI_CLOSURE_PREFILTER;
pub(super) use constants::{DEFAULT_MIN_SCORING_SIDE_KM, MIN_FAI_TO_FREE_DISTANCE_RATIO, MIN_SIDE};
pub(super) use evaluator::FaiTriangleEvaluator;
pub use types::{FaiTriangleClass, FaiTriangleClosureCacheStats, TraceEvent};

#[cfg(test)]
pub(super) use constants::{FAI_TRIANGLE_CLOSED_MULTIPLIER, FAI_TRIANGLE_OPEN_MULTIPLIER};
