use tengri_geo::PointE5;

use super::super::RoutePoint;

#[derive(Debug, Clone)]
pub(super) struct FreeDistanceScore {
    pub(super) turnpoints: Vec<RoutePoint>,
}

pub(super) fn route_point(idx: usize, point: &PointE5) -> RoutePoint {
    RoutePoint {
        idx,
        lat: point.lat,
        lon: point.lon,
    }
}
