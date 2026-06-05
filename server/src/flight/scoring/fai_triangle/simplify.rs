use crate::flight::types::Track;
use tengri_geo::{project_track_points_m, rdp_indexes_with_chord_cap};

/// Simplify a track for scoring and cap the distance between kept points.
///
/// The chord cap forces intermediate points to be kept on long straight
/// segments. Without it, plain RDP may keep only the endpoints, losing
/// potential scorer turnpoints that lie mid-chord.
pub(crate) fn simplify_track_for_scoring_with_chord_cap(
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
