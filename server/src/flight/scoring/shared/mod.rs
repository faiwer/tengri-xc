use crate::flight::types::TrackPoint;
use crate::geo::haversine_m;
impl Point {
    pub(super) fn from_track_point(point: &TrackPoint) -> Self {
        Self::new(point.lat, point.lon)
    }

    pub(super) fn distance(self, other: &Self) -> f64 {
        distance_fcc_m(self, *other)
    }

    pub(super) fn distance_haversine(self, other: &Self) -> f64 {
        haversine_m(self.lat, self.lon, other.lat, other.lon)
    }
}
