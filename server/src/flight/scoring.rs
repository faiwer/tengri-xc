use tengri_formats::Track;
use tengri_geo::PointE5;
pub use tengri_scoring::{
    FAI_CLOSURE_PREFILTER, FaiTriangleLazyAudit, FaiTriangleLazySkipReason, OlcTriangleClass,
    Route, RouteClosure, RouteCylinderMode, RouteEvaluation, RouteFix, RoutePoint, RouteSubType,
    RouteType, RouteWaypoint, ScoringError, ScoringOutcome, TraceEvent, TriangleClosureCacheStats,
};

pub fn evaluate_routes(track: &Track) -> ScoringOutcome<RouteEvaluation> {
    tengri_scoring::evaluate_routes(&scoring_track(track))
}

pub fn evaluate_free_distance(track: &Track) -> ScoringOutcome<Route> {
    tengri_scoring::evaluate_free_distance(&scoring_track(track))
}

pub fn evaluate_free_triangle(track: &Track) -> ScoringOutcome<Route> {
    tengri_scoring::evaluate_free_triangle(&scoring_track(track))
}

pub fn evaluate_free_triangle_lazy(track: &Track, free_distance_m: u32) -> ScoringOutcome<Route> {
    tengri_scoring::evaluate_free_triangle_lazy(&scoring_track(track), free_distance_m)
}

pub fn evaluate_fai_triangle(
    track: &Track,
    class: Option<OlcTriangleClass>,
) -> ScoringOutcome<Route> {
    tengri_scoring::evaluate_fai_triangle(&scoring_track(track), class)
}

pub fn evaluate_fai_triangle_lazy(
    track: &Track,
    free_distance_m: u32,
    audit: Option<&mut FaiTriangleLazyAudit>,
    trace: Option<&mut dyn FnMut(&TraceEvent)>,
) -> ScoringOutcome<Route> {
    tengri_scoring::evaluate_fai_triangle_lazy(&scoring_track(track), free_distance_m, audit, trace)
}

pub(crate) fn scoring_track(track: &Track) -> tengri_scoring::ScoringTrack {
    tengri_scoring::ScoringTrack {
        points: track.points.iter().map(PointE5::from_e5_coords).collect(),
    }
}

#[cfg(test)]
mod tests {
    use tengri_formats::TrackPoint;

    use super::*;

    fn point(time: u32, lat: i32, lon: i32) -> TrackPoint {
        TrackPoint {
            time,
            lat,
            lon,
            geo_alt: 0,
            pressure_alt: None,
            tas: None,
        }
    }

    #[test]
    fn returns_one_outcome_per_scorable_route_type() {
        // Near-equilateral triangle (corners A/B/C, each leg ≈111 km) with tight
        // closure (~55 m). Wobble points on each leg deviate ~11 km from the
        // straight line, so they survive RDP and the free-distance DP gets ≥5 fixes.
        let track = Track {
            start_time: 0,
            points: vec![
                point(0, 0, 0),            // corner A — start / closure
                point(1, 40_000, 10_000),  // leg A→B wobble
                point(2, 60_000, 40_000),  // leg A→B wobble
                point(3, 90_000, 45_000),  // leg A→B wobble
                point(4, 100_000, 50_000), // corner B — first turn
                point(5, 80_000, 70_000),  // leg B→C wobble
                point(6, 40_000, 90_000),  // leg B→C wobble
                point(7, 10_000, 95_000),  // leg B→C wobble
                point(8, 0, 100_000),      // corner C — second turn
                point(9, 500, 250),        // finish — ≈55 m from A
            ],
        };

        let evaluated = match evaluate_routes(&track) {
            ScoringOutcome::Answer(evaluated) => evaluated,
            other => panic!("expected scored routes, got {other:?}"),
        };

        assert_eq!(evaluated.routes.len(), RouteType::SCORABLE.len());
        for (outcome, route_type) in evaluated.routes.iter().zip(RouteType::SCORABLE) {
            match (route_type, outcome) {
                (RouteType::FreeDistance, ScoringOutcome::Answer(route)) => {
                    assert_eq!(route.route_type, RouteType::FreeDistance);
                    assert!(route.optimal);
                }
                (RouteType::FaiTriangle, ScoringOutcome::Answer(route)) => {
                    assert_eq!(route.route_type, RouteType::FaiTriangle);
                    assert!(route.optimal);
                    assert!(route.distance > 0);
                }
                (_, ScoringOutcome::Answer(route)) => assert_eq!(route.route_type, route_type),
                (_, ScoringOutcome::NoAnswer) => {}
                (_, ScoringOutcome::Error(error)) => panic!("{route_type:?} failed: {error}"),
            }
        }
    }
}
