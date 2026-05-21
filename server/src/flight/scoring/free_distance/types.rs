use crate::flight::types::TrackPoint;

use super::super::RoutePoint;

#[derive(Debug, Clone)]
pub(super) struct FreeDistanceScore {
    pub(super) distance_m: f64,
    pub(super) turnpoints: Vec<RoutePoint>,
}

pub(super) fn route_point(track_idx: usize, point: &TrackPoint) -> RoutePoint {
    RoutePoint {
        track_idx,
        time: point.time,
        lat: point.lat,
        lon: point.lon,
    }
}
