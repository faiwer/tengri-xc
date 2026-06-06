mod evaluation;
mod fai_triangle;
mod free_distance;
mod free_triangle;
mod olc_triangle;
mod shared;
mod track;
mod types;

pub use evaluation::evaluate_routes;
pub use fai_triangle::{
    FAI_CLOSURE_PREFILTER, FaiTriangleLazyAudit, FaiTriangleLazySkipReason, OlcTriangleClass,
    TraceEvent, TriangleClosureCacheStats, evaluate_fai_triangle, evaluate_fai_triangle_lazy,
};
pub use free_distance::evaluate_free_distance;
pub use free_triangle::{evaluate_free_triangle, evaluate_free_triangle_lazy};
pub use track::ScoringTrack;
pub use types::{
    Route, RouteClosure, RouteCylinderMode, RouteFix, RoutePoint, RouteSubType, RouteType,
    RouteWaypoint, ScoringError,
};
pub use types::{RouteEvaluation, ScoringOutcome};
