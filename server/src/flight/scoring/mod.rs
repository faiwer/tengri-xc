mod fai_triangle;
mod free_distance;
mod free_triangle;
mod shared;
mod types;

use crate::flight::types::Track;

pub use fai_triangle::{
    FAI_CLOSURE_PREFILTER, FaiTriangleClass, FaiTriangleClosureCacheStats, FaiTriangleLazyAudit,
    FaiTriangleLazySkipReason, TraceEvent, evaluate_fai_triangle, evaluate_fai_triangle_lazy,
};
pub use free_distance::evaluate_free_distance;
pub use free_triangle::{
    evaluate_free_triangle, evaluate_xcontest_free_triangle,
    evaluate_xcontest_free_triangle_bounded,
};
pub use shared::simplify::{simplify_track, simplify_track_for_scoring_with_chord_cap};
pub(crate) use types::{IndexedTrackPoint, RouteClosure, RouteSubType, ScoringError};
pub use types::{Route, RouteEvaluation, RouteType, RouteWaypoint, ScoringOutcome};

pub fn evaluate_routes(track: &Track) -> ScoringOutcome<RouteEvaluation> {
    let free_distance = match evaluate_free_distance(track) {
        ScoringOutcome::Answer(route) => route,
        ScoringOutcome::NoAnswer => {
            return ScoringOutcome::Error(ScoringError::SolverFailed {
                route_type: RouteType::FreeDistance,
                reason: "no free-distance route found",
            });
        }
        ScoringOutcome::Error(error) => return ScoringOutcome::Error(error),
    };
    let routes = std::thread::scope(|scope| {
        let handles: Vec<_> = RouteType::SCORABLE
            .into_iter()
            .map(|route_type| {
                let free_distance = free_distance.clone();
                scope.spawn(move || evaluate_route(track, route_type, &free_distance))
            })
            .collect();

        handles
            .into_iter()
            .map(|handle| handle.join().expect("route evaluation thread panicked"))
            .collect()
    });

    ScoringOutcome::Answer(RouteEvaluation { routes })
}

fn evaluate_route(
    track: &Track,
    route_type: RouteType,
    free_distance: &Route,
) -> ScoringOutcome<Route> {
    match route_type {
        RouteType::FreeDistance => ScoringOutcome::Answer(free_distance.clone()),
        RouteType::FreeTriangle => evaluate_free_triangle(track),
        RouteType::FaiTriangle => {
            evaluate_fai_triangle_lazy(track, free_distance.distance, None, None)
        }
        RouteType::Task => ScoringOutcome::NoAnswer,
    }
}

#[cfg(test)]
mod tests {
    use crate::flight::types::TrackPoint;

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
        // Near-equilateral triangle with tight closure: three ~111 km legs each
        // exceed the 28 % FAI minimum, closure ~55 m << the 20 % Open threshold.
        // Intermediate points along each leg give the free-distance DP solver
        // enough candidates to produce an Answer.
        let track = Track {
            start_time: 0,
            points: vec![
                point(0, 0, 0), // start (closure point)
                point(1, 25_000, 12_500),
                point(2, 50_000, 25_000),
                point(3, 75_000, 37_500),
                point(4, 100_000, 50_000), // first turn
                point(5, 75_000, 62_500),
                point(6, 50_000, 75_000),
                point(7, 25_000, 87_500),
                point(8, 0, 100_000), // second turn
                point(9, 500, 250),   // finish — ≈55 m from start
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
