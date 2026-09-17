use tengri_formats::{Track, TrackPoint};

use crate::flight::{Route, RouteClosure, RoutePoint, RouteSubType, RouteType, RouteWaypoint};

pub(super) fn waypoint(lat: i32, lon: i32) -> RouteWaypoint {
    RouteWaypoint::Point {
        fix: RoutePoint { idx: 0, lat, lon },
    }
}

pub(super) fn triangle(sub_type: RouteSubType, closure: Option<RouteClosure>) -> Route {
    Route {
        id: 1,
        flight_id: "TESTFLT0".to_owned(),
        route_type: RouteType::FreeTriangle,
        sub_type,
        turnpoints: vec![
            waypoint(42_50000, 74_00000),
            waypoint(42_70000, 74_30000),
            waypoint(42_40000, 74_50000),
        ],
        leg_distances: vec![10_000, 10_000, 10_000],
        distance: 30_000,
        score: 42.0,
        factor: 1.4,
        optimal: true,
        closure,
        scored_ms: 12,
    }
}

/// Runs through the turnpoints of [`triangle`], so its waypoints sit inside the
/// track's bounding box.
pub(super) fn sample_track() -> Track {
    Track {
        start_time: 0,
        points: (0..600)
            .map(|i| TrackPoint {
                time: i as u32,
                lat: 42_40000 + i * 50,
                lon: 74_00000 + i * 83,
                geo_alt: 20_000,
                pressure_alt: None,
                tas: None,
            })
            .collect(),
    }
}
