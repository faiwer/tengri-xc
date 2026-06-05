use super::approx::project_track_points_m;
use super::point_e5::HasE5Coords;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn distance_sq(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }
}

/// Ramer–Douglas–Peucker algorithm. Simplifies a track by removing points that
/// are close to the straight line between the two adjacent points. Tolerance is
/// the maximum distance between a point and the line between the two adjacent
/// points. It preserves the shape of the track, but the distance between two
/// consecutive points might be extreme.
pub fn rdp(points: &[Point], tolerance: f64) -> Vec<Point> {
    rdp_with_chord_cap(points, tolerance, None)
}

/// The same as `rdp`, but with a maximum chord length between kept points.
/// - `chord_cap_m`: the returned track must have a point at least each
///   `chord_cap_m` meters (if the given track has them)
pub fn rdp_with_chord_cap(
    points: &[Point],
    tolerance: f64,
    chord_cap_m: Option<f64>,
) -> Vec<Point> {
    rdp_indexes_with_chord_cap(points, tolerance, chord_cap_m)
        .into_iter()
        .map(|idx| points[idx])
        .collect()
}

pub enum RdpCapped {
    Complete(Vec<usize>),
    TooMany,
}

/// The same as `rdp_indexes`, but with a maximum number of points. Once RDP
/// reaches the maximum number of points, it returns `TooMany`.
pub fn rdp_indexes_capped(points: &[Point], tolerance: f64, max_points: usize) -> RdpCapped {
    rdp_indexes_with_chord_cap_capped(points, tolerance, None, max_points)
}

/// The RDP implementation with a maximum chord length between kept points.
/// - `tolerance`: the biggest allowed perpendicular distance between a point
///   and the chord between the two adjacent points.
/// - `chord_cap_m`: the returned track must have a point at least each `chord_cap_m`
///   meters (if the given track has them)
pub fn rdp_indexes_with_chord_cap(
    points: &[Point],
    tolerance: f64,
    chord_cap_m: Option<f64>,
) -> Vec<usize> {
    let n = points.len();
    if n <= 2 {
        return (0..n).collect();
    }

    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let chord_cap_sq = chord_cap_m.map(|cap| cap * cap);
    simplify_range(
        points,
        0,
        n - 1,
        tolerance * tolerance,
        chord_cap_sq,
        &mut keep,
    );

    keep.into_iter()
        .enumerate()
        .filter_map(|(idx, keep)| keep.then_some(idx))
        .collect()
}

pub fn simplify_track_for_scoring<P: HasE5Coords>(points: &[P], tolerance_m: f64) -> Vec<usize> {
    let n = points.len();
    if n <= 2 {
        return (0..n).collect();
    }
    rdp_indexes_with_chord_cap(&project_track_points_m(points), tolerance_m, None)
}

pub fn simplify_track_for_scoring_with_chord_cap<P: HasE5Coords>(
    points: &[P],
    tolerance_m: f64,
    chord_cap_m: f64,
) -> Vec<usize> {
    let n = points.len();
    if n <= 2 {
        return (0..n).collect();
    }
    rdp_indexes_with_chord_cap(
        &project_track_points_m(points),
        tolerance_m,
        Some(chord_cap_m),
    )
}

/// The RDP implementation limited by a maximum number of points.
/// - `tolerance`: the biggest allowed perpendicular distance between a point
///   and the chord between the two adjacent points.
/// - `chord_cap_m`: the track must have a point at least each `chord_cap_m`
///   meters (if the given track has them)
/// - `max_points`: the maximum number of points to keep. If the track has more
///   points than `max_points`, it returns `TooMany`.
fn rdp_indexes_with_chord_cap_capped(
    points: &[Point],
    tolerance: f64,
    chord_cap_m: Option<f64>,
    max_points: usize,
) -> RdpCapped {
    let n = points.len();
    if n <= 2 {
        return RdpCapped::Complete((0..n).collect());
    }

    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut kept_count = 2;
    let chord_cap_sq = chord_cap_m.map(|cap| cap * cap);
    if !simplify_range_capped(
        points,
        0,
        n - 1,
        tolerance * tolerance,
        chord_cap_sq,
        &mut keep,
        &mut kept_count,
        max_points,
    ) {
        return RdpCapped::TooMany;
    }

    RdpCapped::Complete(
        keep.into_iter()
            .enumerate()
            .filter_map(|(idx, keep)| keep.then_some(idx))
            .collect(),
    )
}

fn simplify_range(
    points: &[Point],
    first: usize,
    last: usize,
    tolerance_sq: f64,
    chord_cap_sq: Option<f64>,
    keep: &mut [bool],
) {
    if last <= first + 1 {
        return;
    }

    let mut farthest_idx = first + 1;
    let mut farthest_sq = 0.0;
    for idx in first + 1..last {
        let distance_sq = point_segment_distance_sq(points[idx], points[first], points[last]);
        if distance_sq > farthest_sq {
            farthest_sq = distance_sq;
            farthest_idx = idx;
        }
    }

    let perp_exceeds = farthest_sq > tolerance_sq;
    let chord_exceeds =
        chord_cap_sq.is_some_and(|cap| points[first].distance_sq(points[last]) > cap);

    if !perp_exceeds && !chord_exceeds {
        return;
    }

    let split = if perp_exceeds {
        farthest_idx
    } else {
        (first + last) / 2
    };

    keep[split] = true;
    simplify_range(points, first, split, tolerance_sq, chord_cap_sq, keep);
    simplify_range(points, split, last, tolerance_sq, chord_cap_sq, keep);
}

#[allow(clippy::too_many_arguments)]
fn simplify_range_capped(
    points: &[Point],
    first: usize,
    last: usize,
    tolerance_sq: f64,
    chord_cap_sq: Option<f64>,
    keep: &mut [bool],
    kept_count: &mut usize,
    max_points: usize,
) -> bool {
    if *kept_count > max_points {
        return false;
    }
    if last <= first + 1 {
        return true;
    }

    let mut farthest_idx = first + 1;
    let mut farthest_sq = 0.0;
    for idx in first + 1..last {
        let distance_sq = point_segment_distance_sq(points[idx], points[first], points[last]);
        if distance_sq > farthest_sq {
            farthest_sq = distance_sq;
            farthest_idx = idx;
        }
    }

    let perp_exceeds = farthest_sq > tolerance_sq;
    let chord_exceeds =
        chord_cap_sq.is_some_and(|cap| points[first].distance_sq(points[last]) > cap);

    if !perp_exceeds && !chord_exceeds {
        return true;
    }

    let split = if perp_exceeds {
        farthest_idx
    } else {
        (first + last) / 2
    };

    if !keep[split] {
        keep[split] = true;
        *kept_count += 1;
        if *kept_count > max_points {
            return false;
        }
    }

    simplify_range_capped(
        points,
        first,
        split,
        tolerance_sq,
        chord_cap_sq,
        keep,
        kept_count,
        max_points,
    ) && simplify_range_capped(
        points,
        split,
        last,
        tolerance_sq,
        chord_cap_sq,
        keep,
        kept_count,
        max_points,
    )
}

/// Calculate the squared distance between a point and a segment. This is a
/// plain algebra, it does not take into account the Earth's curvature. Project
/// the points into a local metre plane BEFORE calculating the distance.
fn point_segment_distance_sq(point: Point, start: Point, end: Point) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    if dx == 0.0 && dy == 0.0 {
        return point.distance_sq(start);
    }

    let t = (((point.x - start.x) * dx + (point.y - start.y) * dy) / (dx * dx + dy * dy))
        .clamp(0.0, 1.0);
    let projected = Point::new(start.x + t * dx, start.y + t * dy);
    point.distance_sq(projected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_line_keeps_only_endpoints() {
        let points: Vec<_> = (0..10).map(|idx| Point::new(idx as f64, 0.0)).collect();

        assert_eq!(rdp_indexes_with_chord_cap(&points, 1.0, None), vec![0, 9]);
        assert_eq!(
            rdp(&points, 1.0),
            vec![Point::new(0.0, 0.0), Point::new(9.0, 0.0)]
        );
    }

    #[test]
    fn corner_survives_simplification() {
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(100.0, 200.0),
        ];

        let simplified = rdp_indexes_with_chord_cap(&points, 10.0, None);

        assert!(simplified.contains(&1));
        assert_eq!(simplified.first(), Some(&0));
        assert_eq!(simplified.last(), Some(&3));
    }

    #[test]
    fn chord_cap_forces_midpoints_on_straight_line() {
        let points: Vec<_> = (0..100)
            .map(|idx| Point::new(idx as f64 * 50.0, 0.0))
            .collect();

        let kept = rdp_indexes_with_chord_cap(&points, 10.0, Some(200.0));

        assert_eq!(kept.first(), Some(&0));
        assert_eq!(kept.last(), Some(&99));
        assert!(kept.len() > 20);
        for window in kept.windows(2) {
            let chord = points[window[0]].distance_sq(points[window[1]]).sqrt();
            assert!(chord <= 200.0 + 1e-9, "chord {chord} > cap 200");
        }
    }

    #[test]
    fn chord_cap_does_not_double_count_shape_vertices() {
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(100.0, 200.0),
        ];

        assert_eq!(
            rdp_indexes_with_chord_cap(&points, 10.0, Some(250.0)),
            rdp_indexes_with_chord_cap(&points, 10.0, None)
        );
    }

    #[test]
    fn chord_cap_none_matches_legacy() {
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(80.0, 20.0),
            Point::new(120.0, 120.0),
            Point::new(240.0, 130.0),
            Point::new(320.0, 0.0),
        ];

        assert_eq!(
            rdp_indexes_with_chord_cap(&points, 25.0, None),
            rdp_indexes_with_chord_cap(&points, 25.0, None)
        );
        assert_eq!(rdp_with_chord_cap(&points, 25.0, None), rdp(&points, 25.0));
    }

    #[test]
    fn chord_cap_cannot_subdivide_below_input_resolution() {
        let points = vec![Point::new(0.0, 0.0), Point::new(1000.0, 0.0)];

        assert_eq!(
            rdp_indexes_with_chord_cap(&points, 10.0, Some(100.0)),
            vec![0, 1]
        );
    }
}
