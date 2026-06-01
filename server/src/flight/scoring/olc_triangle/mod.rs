mod bounds;
mod closure;
mod constants;
mod evaluator;
mod geometry;
mod types;

pub(super) use evaluator::FaiTriangleEvaluator;
pub(super) use types::TriangleOptions;
pub use types::{FaiTriangleClass, FaiTriangleClosureCacheStats, TraceEvent};
