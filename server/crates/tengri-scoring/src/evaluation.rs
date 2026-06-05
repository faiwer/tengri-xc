use std::time::Instant;

use crate::{
    Route, RouteEvaluation, RouteType, ScoringError, ScoringOutcome, ScoringTrack,
    evaluate_fai_triangle_lazy, evaluate_free_distance, evaluate_free_triangle_lazy,
};

pub fn evaluate_routes(track: &ScoringTrack) -> ScoringOutcome<RouteEvaluation> {
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
    track: &ScoringTrack,
    route_type: RouteType,
    free_distance: &Route,
) -> ScoringOutcome<Route> {
    match route_type {
        RouteType::FreeDistance => ScoringOutcome::Answer(free_distance.clone()),
        RouteType::FreeTriangle => evaluate_free_triangle_lazy(track, free_distance.distance),
        RouteType::FaiTriangle => {
            evaluate_fai_triangle_lazy(track, free_distance.distance, None, None)
        }
        RouteType::Task => ScoringOutcome::NoAnswer,
    }
}
