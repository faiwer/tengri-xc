mod fai_triangle;
mod free_distance;
mod free_triangle;
mod olc_triangle;
mod types;

use std::time::Instant;

use crate::flight::types::Track;

pub use fai_triangle::{
    FAI_CLOSURE_PREFILTER, FaiTriangleLazyAudit, FaiTriangleLazySkipReason, OlcTriangleClass,
    TraceEvent, TriangleClosureCacheStats, evaluate_fai_triangle, evaluate_fai_triangle_lazy,
};
pub use free_distance::evaluate_free_distance;
pub use free_triangle::evaluate_free_triangle;
pub(crate) use types::{IndexedTrackPoint, RouteClosure, RouteSubType, ScoringError};
pub use types::{Route, RouteEvaluation, RouteType, RouteWaypoint, ScoringOutcome};

pub fn evaluate_routes(track: &Track) -> ScoringOutcome<RouteEvaluation> {
    let t = Instant::now();
    let mut free_distance = match evaluate_free_distance(track) {
        ScoringOutcome::Answer(route) => route,
        ScoringOutcome::NoAnswer => {
            return ScoringOutcome::Error(ScoringError::SolverFailed {
                route_type: RouteType::FreeDistance,
                reason: "no free-distance route found",
            });
        }
        ScoringOutcome::Error(error) => return ScoringOutcome::Error(error),
    };
    free_distance.scored_ms = t.elapsed().as_millis() as u32;

    let routes = std::thread::scope(|scope| {
        let handles: Vec<_> = RouteType::SCORABLE
            .into_iter()
            .map(|route_type| {
                let free_distance = free_distance.clone();
                scope.spawn(move || {
                    if route_type == RouteType::FreeDistance {
                        return ScoringOutcome::Answer(free_distance);
                    }
                    let t = Instant::now();
                    let outcome = evaluate_route(track, route_type, &free_distance);
                    let ms = t.elapsed().as_millis() as u32;
                    outcome.map_answer(|mut route| {
                        route.scored_ms = ms;
                        route
                    })
                })
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
