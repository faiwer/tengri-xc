pub mod backfill;
pub mod compact;
pub mod etag;
pub(crate) mod geo_text;
pub mod gpx;
pub mod igc;
pub mod ingest;
pub mod kml;
pub mod kmz;
pub mod metadata;
pub mod scoring;
pub mod store;
pub mod tengri;
pub mod timezone;
pub mod types;
pub mod window;

pub use compact::{CompactError, CompactTrack, decode, encode};
pub use etag::etag_for;
pub use gpx::GpxError;
pub use igc::IgcError;
pub use kml::KmlError;
pub use kmz::KmzError;
pub use metadata::Metadata;
pub(crate) use scoring::RouteSubType;
pub use scoring::{
    FAI_CLOSURE_PREFILTER, FaiTriangleClass, FaiTriangleClosureCacheStats, FaiTriangleLazyAudit,
    FaiTriangleLazySkipReason, Route, RouteEvaluation, RouteType, RouteWaypoint, ScoringOutcome,
    TraceEvent, evaluate_fai_triangle, evaluate_fai_triangle_lazy, evaluate_free_distance,
    evaluate_free_triangle, evaluate_routes, evaluate_xcontest_free_triangle,
    evaluate_xcontest_free_triangle_bounded, simplify_track,
    simplify_track_for_scoring_with_chord_cap,
};
pub use tengri::{TengriError, TengriFile};
pub use types::{Track, TrackPoint};
pub use window::{FlightWindow, find_flight_window};
