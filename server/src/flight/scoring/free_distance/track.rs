use crate::flight::types::Track;

use super::types::FreeDistanceScore;

/// Track wrapper used by the free-distance scorer.
///
/// The scorer does not need to evaluate repeated fixes that have the exact same
/// latitude and longitude as the previous fix, even when their timestamps are
/// different. Those duplicates are valid track data, but they add search work
/// without changing any possible route distance, so this wrapper builds a
/// deduplicated working track before scoring.
///
/// Callers still need route indexes from the original uploaded track. To keep
/// that contract, the wrapper remembers which original fix each working fix
/// came from and rewrites the winning route back to source indexes before
/// returning it.
pub(super) struct ScoringTrack<'a> {
    source: &'a Track,
    deduped: Option<Track>,
    index_map: Vec<usize>,
}

impl<'a> ScoringTrack<'a> {
    pub(super) fn new(source: &'a Track) -> Self {
        let mut points = Vec::with_capacity(source.points.len());
        let mut index_map = Vec::with_capacity(source.points.len());
        let mut previous_position = None;

        for (idx, point) in source.points.iter().copied().enumerate() {
            let position = (point.lat, point.lon);
            if previous_position == Some(position) {
                continue;
            }
            previous_position = Some(position);
            points.push(point);
            index_map.push(idx);
        }

        if points.len() == source.points.len() {
            Self {
                source,
                deduped: None,
                index_map,
            }
        } else {
            Self {
                source,
                deduped: Some(Track {
                    start_time: source.start_time,
                    points,
                }),
                index_map,
            }
        }
    }

    pub(super) fn track(&self) -> &Track {
        self.deduped.as_ref().unwrap_or(self.source)
    }

    pub(super) fn remap_score(&self, mut score: FreeDistanceScore) -> FreeDistanceScore {
        if self.deduped.is_none() {
            return score;
        }

        for point in &mut score.turnpoints {
            point.track_idx = self.index_map[point.track_idx];
        }
        score
    }
}
