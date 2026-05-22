use crate::flight::types::TrackPoint;

use super::super::IndexedTrackPoint;

#[derive(Debug, Clone)]
pub(super) struct FreeDistanceScore {
    pub(super) turnpoints: Vec<IndexedTrackPoint>,
}

pub(super) fn route_point(track_idx: usize, point: &TrackPoint) -> IndexedTrackPoint {
    IndexedTrackPoint {
        track_idx,
        point: *point,
    }
}
