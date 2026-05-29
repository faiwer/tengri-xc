use super::constants::MIN_SIDE;
use super::geometry::{Box, dedupe_points, push_unique_point};

/// Finds the maximum possible FAI distance given three bounding boxes (one per
/// turnpoint range).
///
/// - max_triangle_distance: result of `max_triangle_distance`
/// - boxes: boxes of the vertices of the current triangle candidate
/// - min_scoring_side_km: bail out if the maximum side length of the triangle
///   is less than this value
pub(super) fn max_fai_distance(boxes: [Box; 3], min_scoring_side_km: f64) -> f64 {
    let max_triangle_distance = max_triangle_distance(boxes);
    let min_tri_distance = min_distance_bw_three_boxes(boxes);
    let max_ab = max_distance_bw_two_boxes([boxes[0], boxes[1]]);
    let max_bc = max_distance_bw_two_boxes([boxes[1], boxes[2]]);
    let max_ca = max_distance_bw_two_boxes([boxes[2], boxes[0]]);
    // The upper bound on the shortest leg of the given 3-boxes
    let max_shortest_side = max_ab.min(max_bc).min(max_ca);

    if max_shortest_side < min_scoring_side_km {
        return 0.0; // Too small to score.
    }

    // Assume the shortest leg is 28%, how big is 100%?
    let max_distance = max_shortest_side / MIN_SIDE;
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
                let distance = a.distance(&b) + b.distance(&c) + c.distance(&a);
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
                let distance = a.distance(&b) + b.distance(&c) + c.distance(&a);
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
            let distance = a.distance(&b);
            if distance > best {
                best = distance;
            }
        }
    }
    best
}
