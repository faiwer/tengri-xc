use super::geometry::{Box, dedupe_points, push_unique_point};

/// Finds the maximum possible FAI distance given three bounding boxes (one per
/// turnpoint range).
///
/// - max_triangle_distance: result of `max_triangle_distance`
/// - boxes: boxes of the vertices of the current triangle candidate
/// - min_scoring_side_km: bail out if the maximum side length of the triangle
///   is less than this value
pub(super) fn max_fai_distance(
    boxes: [Box; 3],
    min_side: Option<f64>,
    min_scoring_side_km: Option<f64>,
) -> f64 {
    let max_triangle_distance = max_triangle_distance(boxes);
    let min_tri_distance = min_distance_bw_three_boxes(boxes);
    let max_ab = max_distance_bw_two_boxes([boxes[0], boxes[1]]);
    let max_bc = max_distance_bw_two_boxes([boxes[1], boxes[2]]);
    let max_ca = max_distance_bw_two_boxes([boxes[2], boxes[0]]);
    // The upper bound on the shortest leg of the given 3-boxes
    let max_shortest_side = max_ab.min(max_bc).min(max_ca);

    if let Some(min_scoring_side_km) = min_scoring_side_km
        && max_shortest_side < min_scoring_side_km
    {
        return 0.0; // Too small to score.
    }

    let Some(min_side) = min_side else {
        return max_triangle_distance;
    };

    // Assume the shortest leg sits at the minimum side fraction (28% for FAI).
    let max_distance = max_shortest_side / min_side;
    if max_distance < min_tri_distance {
        // A FAI triangle in these boxes is impossible.
        return 0.0;
    }

    // Take the smaller "triangle" perimeter:
    // - max_triangle_distance — a real (bound-based) triangle, but not a FAI one
    // - max_distance - not a triangle at all, instead an approximation based on
    //   the longest FAI-shortest "leg"
    //
    // Both values are used as upper bounds to filter out such boxes that even
    // theoretically could not form a triangle bigger than the current best.
    max_distance.min(max_triangle_distance)
}

/// Finds the maximum possible arbitrary (not a FAI one) triangle perimeter
/// given three bounding boxes (one per turnpoint range). The triangle is not
/// directly based on the real track fixes, it's based on their groupped box
/// boundaries. I.e., such a triangle might not exist in the track.
fn max_triangle_distance(boxes: [Box; 3]) -> f64 {
    // Find the global extremes
    let min_lat = boxes.iter().map(|bbox| bbox.min_lat).min().unwrap();
    let min_lon = boxes.iter().map(|bbox| bbox.min_lon).min().unwrap();
    let max_lat = boxes.iter().map(|bbox| bbox.max_lat).max().unwrap();
    let max_lon = boxes.iter().map(|bbox| bbox.max_lon).max().unwrap();

    let vertices = boxes.map(Box::vertices);
    let mut path = [Vec::new(), Vec::new(), Vec::new()];
    // Check if any of the boxes intersect with any other box.
    let intersecting =
        (0..boxes.len()).any(|idx| boxes[idx].intersects(boxes[(idx + 1) % boxes.len()]));

    for idx in 0..boxes.len() {
        if intersecting {
            // Heavy scenario. Take all the vertices.
            path[idx] = dedupe_points(vertices[idx].into_iter().collect());
            continue;
        }

        // Push into path[idx] only the vertices that are global corners.
        for vertex in vertices[idx] {
            if (vertex.lat == min_lat || vertex.lat == max_lat)
                && (vertex.lon == min_lon || vertex.lon == max_lon)
            {
                push_unique_point(&mut path[idx], vertex);
            }
        }

        // When a box doesn't have any global corner points, push those that
        // are touching one of the global boundary lines.
        if path[idx].is_empty() {
            for vertex in vertices[idx] {
                if vertex.lat == min_lat
                    || vertex.lat == max_lat
                    || vertex.lon == min_lon
                    || vertex.lon == max_lon
                {
                    push_unique_point(&mut path[idx], vertex);
                }
            }
        }

        // If we still found nothing, push all the vertices.
        if path[idx].is_empty() {
            path[idx] = dedupe_points(vertices[idx].into_iter().collect());
        }
    }

    // O(n^3) where N is 4 at most. So max 64 iterations.
    let mut best = 0.0;
    for &a in &path[0] {
        for &b in &path[1] {
            for &c in &path[2] {
                let distance = a.distance_haversine_km(&b)
                    + b.distance_haversine_km(&c)
                    + c.distance_haversine_km(&a);
                if distance > best {
                    best = distance;
                }
            }
        }
    }

    best
}

/// Finds the minimum possible distance between three bounding boxes. It
/// iterates over all vertices combinations and returns the minimum distance. It
/// makes 64 comparisons (4x4x4).
fn min_distance_bw_three_boxes(boxes: [Box; 3]) -> f64 {
    let vertices = boxes.map(Box::vertices);
    let mut best = f64::INFINITY;
    for a in vertices[0] {
        for b in vertices[1] {
            for c in vertices[2] {
                let distance = a.distance_haversine_km(&b)
                    + b.distance_haversine_km(&c)
                    + c.distance_haversine_km(&a);
                if distance < best {
                    best = distance;
                }
            }
        }
    }
    best
}

/// Finds the maximum possible distance between two bounding boxes. It iterates
/// over all vertices combinations and returns the maximum distance. At most 16
/// iterations (4x4).
fn max_distance_bw_two_boxes(boxes: [Box; 2]) -> f64 {
    let vertices = boxes.map(Box::vertices);
    let mut best = 0.0;
    for a in vertices[0] {
        for b in vertices[1] {
            let distance = a.distance_haversine_km(&b);
            if distance > best {
                best = distance;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(lat: i32, lon: i32) -> Box {
        Box {
            min_lat: lat,
            min_lon: lon,
            max_lat: lat,
            max_lon: lon,
        }
    }

    #[test]
    fn zero_when_max_shortest_side_below_min_scoring_side() {
        // Three collinear single-point boxes ~55 km apart. max_shortest_side ≈
        // 55.6 km < 100 km floor → 0.
        let boxes = [pt(0, 0), pt(0, 50_000), pt(0, 100_000)];
        assert_eq!(max_fai_distance(boxes, Some(0.28), Some(100.0)), 0.0);
    }

    #[test]
    fn zero_when_fai_shape_is_impossible() {
        // Three nearly collinear single-point boxes with tiny inter-box gaps.
        // max_shortest_side / MIN_SIDE < min_tri_distance, so no FAI triangle
        // can fit inside these boxes.
        let boxes = [pt(0, 0), pt(0, 1_000), pt(0, 2_000)];
        assert_eq!(max_fai_distance(boxes, Some(0.28), Some(0.0)), 0.0);
    }

    #[test]
    fn positive_bound_for_valid_triangle_boxes() {
        // Three well-separated single-point boxes forming a roughly equilateral
        // triangle (~100 km sides). The bound should be positive and at least
        // as large as the actual triangle perimeter.
        let boxes = [pt(0, 0), pt(0, 90_000), pt(77_942, 45_000)];
        let bound = max_fai_distance(boxes, Some(0.28), Some(1.4));
        assert!(bound > 200.0, "expected bound > 200 km, got {bound}");
    }

    #[test]
    fn max_triangle_distance_non_intersecting_boxes() {
        // Three non-overlapping boxes, each a single point at a triangle
        // corner. The max perimeter should equal the sum of the three pairwise
        // distances.
        let a = pt(0, 0);
        let b = pt(0, 90_000);
        let c = pt(77_942, 45_000);
        let direct = a.vertices()[0].distance_haversine_km(&b.vertices()[0])
            + b.vertices()[0].distance_haversine_km(&c.vertices()[0])
            + c.vertices()[0].distance_haversine_km(&a.vertices()[0]);
        let bound = max_triangle_distance([a, b, c]);
        assert!(
            (bound - direct).abs() < 0.001,
            "expected {direct:.3}, got {bound:.3}"
        );
    }

    #[test]
    fn max_triangle_distance_intersecting_boxes() {
        // Two overlapping boxes and one separated box. The function should
        // still return a finite, positive result rather than panicking.
        let a = Box {
            min_lat: 0,
            min_lon: 0,
            max_lat: 50_000,
            max_lon: 50_000,
        };
        let b = Box {
            min_lat: 25_000,
            min_lon: 25_000,
            max_lat: 75_000,
            max_lon: 75_000,
        };
        let c = pt(0, 200_000);
        let bound = max_triangle_distance([a, b, c]);
        assert!(bound > 0.0);
    }
}
