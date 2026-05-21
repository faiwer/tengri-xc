impl Point {
    pub(super) fn from_track_point(point: &TrackPoint) -> Self {
        Self::new(point.lat, point.lon)
    }
}
