mod types;

use crate::flight::types::{Track, TrackPoint};

pub use types::{RouteEvaluation, RouteKind, RoutePoint, RouteResult};

pub fn evaluate_routes(track: &Track) -> RouteEvaluation {
    let endpoints = stub_endpoints(track);
    let routes = RouteKind::ALL
        .into_iter()
        .map(|kind| RouteResult {
            kind,
            distance_m: 0,
            points: 0.0,
            turnpoints: endpoints.clone(),
            optimal: false,
        })
        .collect();

    RouteEvaluation { routes }
}

fn stub_endpoints(track: &Track) -> Vec<RoutePoint> {
    let Some(first) = track.points.first() else {
        return Vec::new();
    };
    let last_idx = track.points.len() - 1;
    let last = &track.points[last_idx];

    if last_idx == 0 {
        vec![route_point(0, first)]
    } else {
        vec![route_point(0, first), route_point(last_idx, last)]
    }
}

fn route_point(track_idx: usize, point: &TrackPoint) -> RoutePoint {
    RoutePoint {
        track_idx,
        time: point.time,
        lat: point.lat,
        lon: point.lon,
    }
}

#[cfg(test)]
mod tests {
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
    fn returns_one_stub_per_route_kind() {
        let track = Track {
            start_time: 10,
            points: vec![point(10, 100, 200), point(20, 300, 400)],
        };

        let evaluated = evaluate_routes(&track);

        assert_eq!(evaluated.routes.len(), RouteKind::ALL.len());
        for (route, kind) in evaluated.routes.iter().zip(RouteKind::ALL) {
            assert_eq!(route.kind, kind);
            assert_eq!(route.distance_m, 0);
            assert_eq!(route.points, 0.0);
            assert!(!route.optimal);
            assert_eq!(
                route.turnpoints,
                vec![
                    RoutePoint {
                        track_idx: 0,
                        time: 10,
                        lat: 100,
                        lon: 200,
                    },
                    RoutePoint {
                        track_idx: 1,
                        time: 20,
                        lat: 300,
                        lon: 400,
                    },
                ]
            );
        }
    }
}
