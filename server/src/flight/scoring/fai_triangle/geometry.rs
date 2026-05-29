use crate::flight::types::TrackPoint;
use crate::geo::fcc_distance_km;

#[derive(Debug, Clone, Copy)]
pub(super) struct Range {
    /// The index of the first fix in the range in RangeBoxes leaves.
    pub(super) start: usize,
    /// The index of the last fix in the range in RangeBoxes leaves.
    pub(super) end: usize,
}

impl Range {
    pub(super) fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Return the number of fixes in the range.
    pub(super) fn count(self) -> usize {
        self.end - self.start + 1
    }

    /// Return the index of the middle fix in the range.
    pub(super) fn center(self) -> usize {
        self.start + (self.end - self.start) / 2
    }

    /// Return the left half of the range (10-16 -> 10-13).
    pub(super) fn left(self) -> Self {
        Self::new(self.start, self.center())
    }

    /// Return the right half of the range (10-16 -> 14-16).
    pub(super) fn right(self) -> Self {
        Self::new(self.start + (self.end - self.start).div_ceil(2), self.end)
    }
}

/// A special data-structure for fast range queries of bounding boxes. Each
/// parent node contains the combined bounding box of its two direct children.
/// It allows to find the combined bounding box for a range in O(log N).
pub(super) struct RangeBoxes {
    /// The number of leaves in `.tree`.
    leaves: usize,
    /// A segment binary tree of bounding boxes. Each parent node contains the
    /// combined bounding box of its two direct children.
    tree: Vec<Box>,
}

impl RangeBoxes {
    pub(super) fn new(points: &[Point]) -> Self {
        // Since it's a binary tree the number of leaves must be a power of two.
        let leaves = points.len().next_power_of_two();
        // The total number of nodes in the tree is 2 * .leaves.
        let mut tree = vec![Box::empty(); leaves * 2];

        // At first, fill the leaves with the bounding boxes of the points.
        for (idx, point) in points.iter().enumerate() {
            tree[leaves + idx] = Box::from_point(*point);
        }

        // Then, fill the rest of the tree by combining the bounding boxes of
        // the children.
        for idx in (1..leaves).rev() {
            tree[idx] = tree[idx * 2].combine(tree[idx * 2 + 1]);
        }

        Self { leaves, tree }
    }

    /// Return a bounding box that includes all the points in the range.
    ///
    /// ```text
    ///        ___ 1 _____
    ///       /           \
    ///      2             3
    ///    /    \        /    \
    ///   4      5      6      7
    ///  / \    / \    / \    / \
    /// 8   9  10 11  12 13  14 15
    ///
    /// .leaves = 8 (8...15)
    /// Each point's bounding box is a leaf node (idx + .leaves)
    ///
    /// To find the combined bounding box for the range [4..=7], we start at
    /// the leaves and work our way up to the parent nodes. If the parent node
    /// doesn't contain nodes outside of the given range, we can skip the
    /// current level. If not — we .combine() it with the box found thus far
    /// and shift to the other edge by one index. O(log N).
    /// ```
    pub(super) fn query(&self, range: Range) -> Box {
        let mut left = range.start + self.leaves;
        let mut right = range.end + self.leaves;
        let mut result = Box::empty();
        while left <= right {
            // 1) "% 2 == 0" — means we don't need to .combine() it now. We'll
            //    probably do it with the parent node, that contains both the
            //    left and right children.
            // 2) "% 2 == 1" — means we need to .combine() it now, because the
            //    node at the left ("% 2 == 0") is already out of the range.
            if left % 2 == 1 {
                result = result.combine(self.tree[left]);
                // The parent is wrong, because it contains the left child.
                // Shift to the right.
                left += 1;
            }

            // `is_multiple_of` is a non-panic version of "% 2 == 0" check. The
            // logic below is the same as with the branch above, but reversed.
            if right.is_multiple_of(2) {
                result = result.combine(self.tree[right]);
                right -= 1;
            }
            // Level up to the parent nodes.
            left /= 2;
            right /= 2;
        }
        result
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Box {
    pub(super) min_lat: i32,
    pub(super) min_lon: i32,
    pub(super) max_lat: i32,
    pub(super) max_lon: i32,
}

impl Box {
    pub(super) fn empty() -> Self {
        Self {
            min_lat: i32::MAX,
            min_lon: i32::MAX,
            max_lat: i32::MIN,
            max_lon: i32::MIN,
        }
    }

    pub(super) fn from_point(point: Point) -> Self {
        Self {
            min_lat: point.lat,
            min_lon: point.lon,
            max_lat: point.lat,
            max_lon: point.lon,
        }
    }

    /// Extend the existing bounding box to include the other bounding box.
    /// Just push the min/max lat/lon values outwards when needed.
    pub(super) fn combine(self, other: Self) -> Self {
        Self {
            min_lat: self.min_lat.min(other.min_lat),
            min_lon: self.min_lon.min(other.min_lon),
            max_lat: self.max_lat.max(other.max_lat),
            max_lon: self.max_lon.max(other.max_lon),
        }
    }

    /// Return the four Points that are the corners of the bounding box.
    pub(super) fn vertices(self) -> [Point; 4] {
        [
            Point::new(self.min_lat, self.min_lon),
            Point::new(self.min_lat, self.max_lon),
            Point::new(self.max_lat, self.max_lon),
            Point::new(self.max_lat, self.min_lon),
        ]
    }

    /// Return true if the two boxes intersect.
    pub(super) fn intersects(self, other: Self) -> bool {
        self.min_lat <= other.max_lat
            && self.max_lat >= other.min_lat
            && self.min_lon <= other.max_lon
            && self.max_lon >= other.min_lon
    }

    /// Return the area of the bounding box (width * height).
    pub(super) fn area(self) -> f64 {
        let width = (self.max_lon - self.min_lon).abs() as f64;
        let height = (self.max_lat - self.min_lat).abs() as f64;
        width * height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Point {
    pub(super) lat: i32,
    pub(super) lon: i32,
}

impl Point {
    pub(super) fn from_track_point(point: &TrackPoint) -> Self {
        Self::new(point.lat, point.lon)
    }

    pub(super) fn new(lat: i32, lon: i32) -> Self {
        Self { lat, lon }
    }

    pub(super) fn distance(self, other: &Self) -> f64 {
        fcc_distance_km(self.lat, self.lon, other.lat, other.lon)
    }
}

/// Push a point to a vector if it's not already present. O(n)
pub(super) fn push_unique_point(points: &mut Vec<Point>, point: Point) {
    if !points.contains(&point) {
        points.push(point);
    }
}

/// Returns a new vector with the unique points. O(n)
pub(super) fn dedupe_points(points: Vec<Point>) -> Vec<Point> {
    points.into_iter().fold(Vec::new(), |mut unique, point| {
        push_unique_point(&mut unique, point);
        unique
    })
}
