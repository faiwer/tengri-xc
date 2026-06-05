use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use super::geometry::{Box, Point, Range, RangeBoxes};
use super::types::TriangleClosureCacheStats;

/// Branch-and-Bound search for the nearest (Q, W) closure pair.
///
/// ```text
///              B
///             / \
///            /   \
///           /     \
///          /       \
///         /         \
///        /           \
///       A ----------- C
///        \           /
///         \         /
///          Q       W
///          └-------┘
///         closure gap
/// ```
///
/// Splits the prefix `[0..A]` and suffix `[C..n-1]` ranges recursively,
/// pruning sub-problems whose lower-bound distance already exceeds the current
/// best. Pruning uses the same `RangeBoxes` segment tree as the outer B&B.
///
/// A validity-rectangle cache avoids re-running the B&B for queries whose
/// search space is a subset of a previously solved one: if `(q, w, d)` was the
/// answer for `(a₀, c₀)`, then any `(a, c)` with `q ≤ a ≤ a₀` and `c₀ ≤ c ≤ w`
/// has the same exact answer — the new search space is a subset of the old one,
/// and the optimal pair (q, w) is still reachable.
pub(super) struct ClosurePairs {
    range_boxes: RangeBoxes,
    /// Validity-rect cache keyed by `a_idx` (ascending). Allows O(log n)
    /// lower-bound lookup: only rects with `a_idx ≥ a` are candidates for an
    /// exact hit, so `range(a..)` skips the rest immediately.
    rects: RefCell<BTreeMap<usize, Vec<ValidityRect>>>,
    /// Number of B&B calls to trouble-shoot.
    calls: Cell<u64>,
    /// Number of B&B nodes processed.
    nodes: Cell<u64>,
}

/// A computed closure result together with its validity rectangle.
///
/// Valid for any query `(a, c)` where `q_idx ≤ a` and `c_idx ≤ c ≤ w_idx`. The
/// upper bound on `a` is the BTreeMap key this rect is stored under.
#[derive(Clone, Copy)]
struct ValidityRect {
    q_idx: usize,
    w_idx: usize,
    c_idx: usize,
    distance_km: f64,
}

/// The nearest prefix/suffix pair found by the B&B search.
#[derive(Debug, Clone, Copy)]
pub(super) struct ClosurePair {
    pub(super) q_idx: usize,
    pub(super) w_idx: usize,
    pub(super) distance_km: f64,
}

impl ClosurePairs {
    pub(super) fn new(points: &[Point]) -> Self {
        Self {
            range_boxes: RangeBoxes::new(points),
            rects: RefCell::new(BTreeMap::new()),
            calls: Cell::new(0),
            nodes: Cell::new(0),
        }
    }

    /// Find the (Q, W) pair with minimum Haversine distance where Q ≤ A and W ≥
    /// C. Returns `None` when the query is invalid (`c ≤ a` or out of bounds).
    pub(super) fn closest_pair(&self, a: usize, c: usize, points: &[Point]) -> Option<ClosurePair> {
        if a >= points.len() || c >= points.len() || c <= a {
            // Invalid query. Bail out.
            return None;
        }

        if let Some(hit) = self.lookup_cached(a, c) {
            return Some(hit);
        }

        // Cache miss: seed the B&B with the best reachable cached pair so the
        // search can prune aggressively from the start ignoring obviously worse
        // pairs.
        let mut best = self.best_from_cache(a, c, points.len());

        self.calls.set(self.calls.get() + 1);
        // Run the B&B search with our best anchor.
        self.search(
            Range::new(0, a),
            Range::new(c, points.len() - 1),
            points,
            &mut best,
        );

        if best.distance_km.is_finite() {
            let rect = ValidityRect {
                q_idx: best.q_idx,
                w_idx: best.w_idx,
                c_idx: c,
                distance_km: best.distance_km,
            };
            self.rects.borrow_mut().entry(a).or_default().push(rect);
            Some(best)
        } else {
            None
        }
    }

    /// Find the best cached pair `(q, w, d)` where `q ≤ a` and `w ≥ c`. Will be
    /// used as an anchor for the B&B search.
    ///
    /// The returned pair is reachable from the search space `[0..a] ×
    /// [c..n-1]`, so its distance is a valid upper bound for the upcoming B&B.
    /// The query that produced the rect may have been wider or narrower — only
    /// the stored pair coordinates matter here.
    fn best_from_cache(&self, a: usize, c: usize, n: usize) -> ClosurePair {
        let rects = self.rects.borrow();
        let mut best = ClosurePair {
            q_idx: 0,
            w_idx: n - 1,
            distance_km: f64::INFINITY,
        };
        for (_key, bucket) in rects.iter() {
            for r in bucket {
                if r.q_idx <= a && r.w_idx >= c && r.distance_km < best.distance_km {
                    best = ClosurePair {
                        q_idx: r.q_idx,
                        w_idx: r.w_idx,
                        distance_km: r.distance_km,
                    };
                }
            }
        }
        best
    }

    /// Check if a previously computed result covers the query `(a, c)` exactly.
    ///
    /// A stored `(q, w, d)` from `(a₀, c₀)` covers `(a, c)` when
    /// `q ≤ a ≤ a₀` AND `c₀ ≤ c ≤ w`. Any two covering rects must agree on
    /// distance, so the first hit is the answer.
    fn lookup_cached(&self, a: usize, c: usize) -> Option<ClosurePair> {
        let rects = self.rects.borrow();
        for (key, bucket) in rects.range(a..) {
            // Any rect here has a_idx = key ≥ a. When key > c, its c_idx >
            // a_idx > c, so r.c_idx <= c can never hold — break early.
            if *key > c {
                break; // The clouses for >c are not applicable to (a, c).
            }

            for r in bucket {
                if r.q_idx <= a && r.c_idx <= c && c <= r.w_idx {
                    return Some(ClosurePair {
                        q_idx: r.q_idx,
                        w_idx: r.w_idx,
                        distance_km: r.distance_km,
                    });
                }
            }
        }
        None
    }

    /// Recursively split the search space into two halves and prune when the
    /// lower bound exceeds the best known distance.
    fn search(&self, prefix: Range, suffix: Range, points: &[Point], best: &mut ClosurePair) {
        self.nodes.set(self.nodes.get() + 1);
        let lb_km = lower_bound_km(
            self.range_boxes.query(prefix),
            self.range_boxes.query(suffix),
        );
        if lb_km >= best.distance_km {
            // No need to search further: the lower bound based on the bounding
            // boxes is already greater than the best known distance. Bail out.
            return;
        }

        if prefix.count() == 1 && suffix.count() == 1 {
            // Narrowed down to a single point in each range: compute the exact
            // distance and update the best if it's better.
            let d = points[prefix.start].distance_haversine_km(&points[suffix.start]);
            if d < best.distance_km {
                best.distance_km = d;
                best.q_idx = prefix.start;
                best.w_idx = suffix.start;
            }
            return;
        }

        if prefix.count() >= suffix.count() {
            self.search(prefix.left(), suffix, points, best);
            self.search(prefix.right(), suffix, points, best);
        } else {
            self.search(prefix, suffix.right(), points, best);
            self.search(prefix, suffix.left(), points, best);
        }
    }

    pub(super) fn cache_stats(&self) -> TriangleClosureCacheStats {
        TriangleClosureCacheStats {
            cached_closures: self.rects.borrow().values().map(|v| v.len()).sum(),
            cached_prefix_trees: self.calls.get() as usize,
            max_cache_hits_per_lookup: self.nodes.get() as usize,
            ..Default::default()
        }
    }
}

/// A guaranteed lower bound on the minimum Haversine distance between any point
/// in `box_q` and any point in `box_w`.
///
/// Constructs virtual points at the minimum possible lat/lon separation between
/// the two boxes and returns their Haversine distance — which is ≤ the actual
/// Haversine distance for every point pair in the boxes.
fn lower_bound_km(box_q: Box, box_w: Box) -> f64 {
    let lat_gap = (box_q.min_lat - box_w.max_lat)
        .max(box_w.min_lat - box_q.max_lat)
        .max(0);
    let lon_gap = (box_q.min_lon - box_w.max_lon)
        .max(box_w.min_lon - box_q.max_lon)
        .max(0);

    if lat_gap == 0 && lon_gap == 0 {
        return 0.0; // Overlapping boxes: no lower bound.
    }

    // Most-poleward lat across both boxes: has the smallest cos(lat), which
    // minimises the lon component in the FCC formula → guaranteed lower bound.
    let ref_lat = [box_q.min_lat, box_q.max_lat, box_w.min_lat, box_w.max_lat]
        .into_iter()
        .max_by_key(|l| l.abs())
        .unwrap_or(0);

    // Place the two virtual points symmetrically so mid_lat = ref_lat.
    let half = lat_gap / 2;
    Point::new(ref_lat - half, 0).distance_haversine_km(&Point::new(ref_lat + half, lon_gap))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(min_lat: i32, min_lon: i32, max_lat: i32, max_lon: i32) -> Box {
        Box {
            min_lat,
            min_lon,
            max_lat,
            max_lon,
        }
    }

    #[test]
    fn lower_bound_zero_for_overlapping_boxes() {
        let a = bbox(0, 0, 100_000, 100_000);
        let b = bbox(50_000, 50_000, 150_000, 150_000);
        assert_eq!(lower_bound_km(a, b), 0.0);
    }

    #[test]
    fn lower_bound_zero_for_touching_boxes() {
        // Boxes share an edge — still overlapping per the ≤ gap check.
        let a = bbox(0, 0, 100_000, 0);
        let b = bbox(100_000, 0, 200_000, 0);
        assert_eq!(lower_bound_km(a, b), 0.0);
    }

    #[test]
    fn lower_bound_positive_for_lat_separated_boxes() {
        // Boxes separated only in latitude (~111 km per degree).
        let a = bbox(0, 0, 0, 0);
        let b = bbox(100_000, 0, 100_000, 0); // 1° north ≈ 111.2 km
        let lb = lower_bound_km(a, b);
        assert!(
            lb > 0.0 && lb <= 111.3,
            "expected (0, 111.3] km, got {lb:.3}"
        );
    }

    #[test]
    fn lower_bound_positive_for_lon_separated_boxes() {
        // Boxes separated only in longitude on the equator.
        let a = bbox(0, 0, 0, 0);
        let b = bbox(0, 100_000, 0, 100_000); // 1° east ≈ 111.2 km
        let lb = lower_bound_km(a, b);
        assert!(
            lb > 0.0 && lb <= 111.3,
            "expected (0, 111.3] km, got {lb:.3}"
        );
    }

    #[test]
    fn lower_bound_is_not_greater_than_actual_distance() {
        // The bound must never exceed the true Haversine distance between the
        // closest corners — that would make it an invalid lower bound.
        let a = bbox(0, 0, 50_000, 50_000);
        let b = bbox(200_000, 200_000, 250_000, 250_000);
        let lb = lower_bound_km(a, b);
        // Closest corners: a=(0.5°,0.5°) and b=(2°,2°)
        let actual =
            Point::new(50_000, 50_000).distance_haversine_km(&Point::new(200_000, 200_000));
        assert!(
            lb <= actual + 1e-9,
            "lower bound {lb:.6} exceeds actual distance {actual:.6}"
        );
    }
}
