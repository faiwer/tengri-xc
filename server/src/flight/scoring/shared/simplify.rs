use crate::flight::types::Track;
use crate::geo::{
    RdpCapped, project_track_points_m, rdp_indexes, rdp_indexes_capped, rdp_indexes_with_chord_cap,
};

/// Simplify a track with plain RDP and return indexes from the original track.
/// The output track preserves the original track's shape, but the distance
/// between two consecutive points might be extreme.
pub fn simplify_track(track: &Track, tolerance_m: f64) -> Vec<usize> {
    let n = track.points.len();
    if n <= 2 {
        return (0..n).collect();
    }
    rdp_indexes(&project_track_points_m(&track.points), tolerance_m)
}

/// Apply RDP to the track using binary search.
///
/// The goal is to find a tolerance that makes the simplified track contain
/// between `min_points` and `max_points` points. Since RDP is relatively cheap,
/// this helps find a simplified track that balances algorithm time and accuracy.
pub fn simplify_track_to_target_count(
    track: &Track,
    min_points: usize,
    max_points: usize,
    min_tolerance_m: f64,
    max_tolerance_m: f64,
) -> Option<Vec<usize>> {
    let n = track.points.len();
    if n <= 2 {
        return Some((0..n).collect());
    }

    let points = project_track_points_m(&track.points);
    let mut too_dense_m = min_tolerance_m;
    let mut sparse_enough_m = max_tolerance_m;
    let mut densest_complete = None;

    for _ in 0..12 {
        let tolerance_m = (too_dense_m + sparse_enough_m) / 2.0;
        match rdp_indexes_capped(&points, tolerance_m, max_points) {
            RdpCapped::TooMany => {
                too_dense_m = tolerance_m;
            }
            RdpCapped::Complete(candidates) => {
                if candidates.len() < min_points {
                    // Remember the densest result so far to use it as a
                    // fallback if never find > min_points. It's better return
                    // < min_points than fallback to the whole track.
                    if densest_complete
                        .as_ref()
                        .is_none_or(|best: &Vec<usize>| candidates.len() > best.len())
                    {
                        densest_complete = Some(candidates.clone());
                    }
                    sparse_enough_m = tolerance_m;
                    continue;
                }

                return Some(candidates);
            }
        }
    }

    densest_complete
}

/// Simplify a track for scoring and cap the distance between kept points.
///
/// The chord cap forces intermediate points to be kept on long straight
/// segments. Without it, plain RDP may keep only the endpoints, losing
/// potential scorer turnpoints that lie mid-chord.
pub fn simplify_track_for_scoring_with_chord_cap(
    track: &Track,
    tolerance_m: f64,
    chord_cap_m: f64,
) -> Vec<usize> {
    let n = track.points.len();
    if n <= 2 {
        return (0..n).collect();
    }
    rdp_indexes_with_chord_cap(
        &project_track_points_m(&track.points),
        tolerance_m,
        Some(chord_cap_m),
    )
}

#[cfg(test)]
mod tests {
    use crate::flight::types::TrackPoint;

    use super::*;

    fn point(idx: u32, lat: i32, lon: i32) -> TrackPoint {
        TrackPoint {
            time: idx,
            lat,
            lon,
            geo_alt: 0,
            pressure_alt: None,
            tas: None,
        }
    }

    #[test]
    fn straight_line_keeps_only_endpoints() {
        let track = Track {
            start_time: 0,
            points: (0..10).map(|idx| point(idx, idx as i32 * 10, 0)).collect(),
        };

        assert_eq!(simplify_track(&track, 1.0), vec![0, 9]);
    }

    #[test]
    fn chord_cap_forces_straight_line_candidates() {
        let track = Track {
            start_time: 0,
            points: (0..10).map(|idx| point(idx, idx as i32 * 10, 0)).collect(),
        };

        let simplified = simplify_track_for_scoring_with_chord_cap(&track, 1.0, 40.0);

        assert_eq!(simplified.first(), Some(&0));
        assert_eq!(simplified.last(), Some(&9));
        assert!(simplified.len() > simplify_track(&track, 1.0).len());
    }

    #[test]
    fn corner_survives_simplification() {
        let track = Track {
            start_time: 0,
            points: vec![
                point(0, 0, 0),
                point(1, 0, 100),
                point(2, 100, 100),
                point(3, 200, 100),
            ],
        };

        let simplified = simplify_track(&track, 10.0);

        assert!(simplified.contains(&1));
        assert_eq!(simplified.first(), Some(&0));
        assert_eq!(simplified.last(), Some(&3));
    }
}
