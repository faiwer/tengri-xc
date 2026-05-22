use crate::flight::types::TrackPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Point {
    pub(super) lat: i32,
    pub(super) lon: i32,
}

impl Point {
    pub(super) fn from_track_point(point: &TrackPoint) -> Self {
        Self::new(point.lat, point.lon)
    }

    pub(super) fn new(lat: i32, lon: i32) -> Self {
        Self { lat, lon }
    }

    pub(super) fn distance(self, other: &Self) -> f64 {
        distance_fcc_m(self, *other)
    }

}
}
