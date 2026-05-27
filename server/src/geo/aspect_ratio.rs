use std::cmp::Ordering;

use crate::flight::types::Track;

use super::{Point, project_track_points_m, rdp};

const RDP_TOLERANCE_M: f64 = 100.0;

/// Aspect ratio of the minimum-area rotated bounding rectangle around a track.
pub fn track_aspect_ratio(track: &Track) -> Option<f64> {
    if track.points.len() < 3 {
        return None;
    }

    // Consider a track to be lying on a flat surface.
    let projected = project_track_points_m(&track.points);
    let simplified = rdp(&projected, RDP_TOLERANCE_M);

    let hull = convex_hull(simplified);
    if hull.len() < 2 {
        return None;
    }

    let (a, b) = min_rotated_rect_sides(&hull)?;
    let longer = a.max(b);
    let shorter = a.min(b);
    if shorter == 0.0 {
        return Some(f64::INFINITY);
    }

    Some(longer / shorter)
}

/// Computes the convex hull (polygon) of a set of points. O(n log n).
fn convex_hull(mut points: Vec<Point>) -> Vec<Point> {
    points.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(Ordering::Equal))
    });
    points.dedup();

    if points.len() < 3 {
        return points;
    }

    let mut hull = Vec::with_capacity(points.len() * 2);
    for &point in &points {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0 {
            hull.pop();
        }
        hull.push(point);
    }

    let lower_len = hull.len() + 1;
    for &point in points.iter().rev().skip(1) {
        while hull.len() >= lower_len
            && cross(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0
        {
            hull.pop();
        }
        hull.push(point);
    }
    hull.pop();
    hull
}

/// cross(a, b, c) computes the 2D cross product of vectors AB and AC:
/// - Positive: c is to the left of the directed line a -> b
/// - Negative: c is to the right
/// - Zero: c is collinear with a -> b
fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Finds the side lengths of the minimum-area rotated bounding rectangle around
/// a convex hull. O(h²), where h is the number of hull vertices. Shouldn't be a
/// problem since we take into account only the hull vertices (after RDP).
fn min_rotated_rect_sides(hull: &[Point]) -> Option<(f64, f64)> {
    if hull.len() < 2 {
        return None;
    }

    let mut best_area = f64::INFINITY;
    let mut best_sides = (0.0, 0.0);
    for i in 0..hull.len() {
        let a = hull[i];
        let b = hull[(i + 1) % hull.len()];
        let edge_len = (b.x - a.x).hypot(b.y - a.y);
        if edge_len == 0.0 {
            continue;
        }

        let ux = (b.x - a.x) / edge_len;
        let uy = (b.y - a.y) / edge_len;
        let vx = -uy;
        let vy = ux;
        let mut min_u = f64::INFINITY;
        let mut max_u = f64::NEG_INFINITY;
        let mut min_v = f64::INFINITY;
        let mut max_v = f64::NEG_INFINITY;

        for &point in hull {
            let dx = point.x - a.x;
            let dy = point.y - a.y;
            let proj_u = dx * ux + dy * uy;
            let proj_v = dx * vx + dy * vy;
            min_u = min_u.min(proj_u);
            max_u = max_u.max(proj_u);
            min_v = min_v.min(proj_v);
            max_v = max_v.max(proj_v);
        }

        let width = max_u - min_u;
        let height = max_v - min_v;
        let area = width * height;
        if area < best_area {
            best_area = area;
            best_sides = (width, height);
        }
    }

    best_area.is_finite().then_some(best_sides)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_4, TAU};

    use crate::flight::types::TrackPoint;

    use super::*;

    const E5_TO_DEG: f64 = 1.0 / 1e5;
    const KM_PER_DEG_LAT: f64 = 111.13209;
    const KM_PER_DEG_LON_EQUATOR: f64 = 111.41513;
    const BASE_LAT_E5: i32 = 4_700_000;
    const BASE_LON_E5: i32 = 800_000;

    fn track_from(points: &[(u32, i32, i32)]) -> Track {
        Track {
            start_time: points.first().map_or(0, |point| point.0),
            points: points
                .iter()
                .map(|&(time, lat, lon)| TrackPoint {
                    time,
                    lat,
                    lon,
                    geo_alt: 0,
                    pressure_alt: None,
                    tas: None,
                })
                .collect(),
        }
    }

    fn e5_offset(lat_offset_km: f64, lon_offset_km: f64) -> (i32, i32) {
        let lat_deg = lat_offset_km / KM_PER_DEG_LAT;
        let base_lat_deg = BASE_LAT_E5 as f64 * E5_TO_DEG;
        let lon_deg = lon_offset_km / (KM_PER_DEG_LON_EQUATOR * base_lat_deg.to_radians().cos());
        (
            BASE_LAT_E5 + (lat_deg / E5_TO_DEG).round() as i32,
            BASE_LON_E5 + (lon_deg / E5_TO_DEG).round() as i32,
        )
    }

    fn track_from_km(points: &[(f64, f64)]) -> Track {
        let points: Vec<_> = points
            .iter()
            .enumerate()
            .map(|(idx, &(x, y))| {
                let (lat, lon) = e5_offset(y, x);
                (idx as u32, lat, lon)
            })
            .collect();
        track_from(&points)
    }

    #[test]
    fn horizontal_line_is_extremely_elongated() {
        let points: Vec<_> = (0..100).map(|idx| (idx as f64 * 0.1, 0.0)).collect();
        let ratio = track_aspect_ratio(&track_from_km(&points));

        assert!(ratio.is_none_or(|ratio| ratio.is_infinite() || ratio >= 1000.0));
    }

    #[test]
    fn approximate_circle_is_near_square() {
        let points: Vec<_> = (0..64)
            .map(|idx| {
                let angle = idx as f64 / 64.0 * TAU;
                (angle.cos(), angle.sin())
            })
            .collect();
        let ratio = track_aspect_ratio(&track_from_km(&points)).expect("circle has area");

        assert!(
            (1.0..=1.15).contains(&ratio),
            "expected near-square circle bounds, got {ratio}"
        );
    }

    #[test]
    fn tilted_rectangle_preserves_aspect_ratio() {
        let base = [
            (-5.0, -0.5),
            (0.0, -0.5),
            (5.0, -0.5),
            (5.0, 0.0),
            (5.0, 0.5),
            (0.0, 0.5),
            (-5.0, 0.5),
            (-5.0, 0.0),
        ];
        let points: Vec<_> = base
            .into_iter()
            .map(|(x, y)| {
                let rotated_x = x * FRAC_PI_4.cos() - y * FRAC_PI_4.sin();
                let rotated_y = x * FRAC_PI_4.sin() + y * FRAC_PI_4.cos();
                (rotated_x, rotated_y)
            })
            .collect();
        let ratio = track_aspect_ratio(&track_from_km(&points)).expect("rectangle has area");

        assert!(
            (9.0..=11.0).contains(&ratio),
            "expected ratio near 10, got {ratio}"
        );
    }

    #[test]
    fn sparse_track_returns_none() {
        let track = track_from_km(&[(0.0, 0.0), (1.0, 1.0)]);

        assert_eq!(track_aspect_ratio(&track), None);
    }

    #[test]
    fn identical_points_return_none() {
        let track = track_from_km(&[(1.0, 1.0), (1.0, 1.0), (1.0, 1.0)]);

        assert_eq!(track_aspect_ratio(&track), None);
    }
}
