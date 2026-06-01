use std::cmp::Ordering;

use serde::Serialize;

use super::super::RouteSubType;
use super::constants::{OLC_CLOSURE_CLOSED, OLC_CLOSURE_OPEN};
use super::geometry::{Range, RangeBoxes};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaiTriangleClass {
    Open,   // 20% closure gap
    Closed, // 5% closure gap
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TriangleOptions {
    /// The OLC triangle subtype. Only `OlcOpen` and `OlcClosed` are supported.
    pub(crate) sub_type: RouteSubType,
    /// The OLC triangle scoring multiplier. Each km of triangle perimeter
    /// scores this many points.
    pub(crate) multiplier: f64,
    /// The minimum length of each side as a fraction of total triangle distance.
    /// For FAI triangles, this is 28%. For OLC triangles it's None.
    pub(crate) min_side: Option<f64>,
    /// The minimum length of each side as a fraction of the flight's free-distance
    /// result. For FAI triangles, this is 1.4km. For OLC triangles it's None.
    pub(crate) min_scoring_side_km: Option<f64>,
}

impl TriangleOptions {
    pub(crate) fn closure_ratio(self) -> f64 {
        match self.sub_type {
            RouteSubType::OlcOpen => OLC_CLOSURE_OPEN,
            RouteSubType::OlcClosed => OLC_CLOSURE_CLOSED,
            _ => unreachable!("unsupported OLC triangle subtype"),
        }
    }
}

/// Per-node B&B trace event, mirrors the Node upstream `traceCb` shape so
/// traces from both can be diffed line-by-line.
#[derive(Debug, Clone)]
pub struct NodeTraceEvent {
    pub kind: TraceEventKind,
    pub processed: u64,
    pub ranges: [(usize, usize); 3],
    pub bound: f64,
    /// `None` for `Pruned` events (score not computed).
    pub score: Option<f64>,
    pub best: f64,
    pub pending: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEventKind {
    Kept,
    Pruned,
}

/// Emitted once after the B&B loop completes, carrying all diagnostic counters.
#[derive(Debug, Clone, Copy)]
pub struct SummaryTraceEvent {
    pub processed: u64,
    pub current_upper_bound: f64,
    pub closure_cache_stats: FaiTriangleClosureCacheStats,
}

#[derive(Debug, Clone)]
pub enum TraceEvent {
    Node(NodeTraceEvent),
    Summary(SummaryTraceEvent),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FaiTriangleClosureCacheStats {
    pub cached_prefix_trees: usize,
    pub cached_prefix_tree_points: usize,
    pub largest_cached_prefix_tree_points: usize,
    pub cached_closures: usize,
    pub max_cache_hits_per_lookup: usize,
}

/// Everything needed to score a FAI triangle.
#[derive(Debug, Clone, Copy)]
pub(super) struct ScoreInfo {
    pub(super) closure: Closure,
    pub(super) turnpoints: [usize; 3],
    pub(super) score: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Closure {
    pub(super) start_idx: usize,
    pub(super) end_idx: usize,
    pub(super) distance_km: f64,
}

/// B&B work queue item.
#[derive(Debug, Clone)]
pub(super) struct PendingSolution {
    /// The optimistic upper score for any triangle whose turnpoints land inside
    /// A, B, C ranges. Used to decide priority: the heap always pops the node
    /// with the highest bound first.
    pub(super) bound: f64,
    /// The actual score when the three range centers were used as concrete
    /// turnpoints
    pub(super) score: f64,
    /// The full detail of .score
    pub(super) score_info: Option<ScoreInfo>,
    /// A unique ID for the solution for BinaryHeap
    pub(super) id: usize,
    /// A, B, & C ranges.
    pub(super) solution: Solution,
}

impl PendingSolution {
    pub(super) fn to_best(&self) -> BestSolution {
        BestSolution {
            score: self.score,
            score_info: self.score_info,
        }
    }
}

impl Ord for PendingSolution {
    /// A comparator for the BinaryHeap.
    fn cmp(&self, other: &Self) -> Ordering {
        // Max-heap by bound; ties broken in favour of the *newer* id. Mirrors
        // upstream `Solution.contentCompare` + SortedSet.pop(). Empirically
        // critical: with many siblings at the same bound, newer-first behaves
        // like DFS — drills into a leaf, raises `best.score`, prunes the rest.
        // Older-first wastes the iteration budget producing siblings before any
        // leaf scores.
        self.bound
            .total_cmp(&other.bound)
            .then_with(|| self.id.cmp(&other.id))
    }
}

// `impl Ord` requires `impl PartialOrd`
impl PartialOrd for PendingSolution {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PendingSolution {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for PendingSolution {}

/// A real FAI triangle based on real GPS-fixes. It passes all the FAI triangle
/// rules.
#[derive(Debug, Clone, Copy)]
pub(super) struct BestSolution {
    /// The score of the best FAI triangle.
    pub(super) score: f64,
    /// The turnpoints and closure of the best FAI triangle.
    pub(super) score_info: Option<ScoreInfo>,
}

impl BestSolution {
    pub(super) fn empty() -> Self {
        Self {
            score: 0.0,
            score_info: None,
        }
    }
}

pub(super) fn to_trace_ranges(solution: &Solution) -> [(usize, usize); 3] {
    solution.ranges.map(|range| (range.start, range.end))
}

/// A container for three ranges representing triangle vertices.
#[derive(Debug, Clone, Copy)]
pub(super) struct Solution {
    pub(super) ranges: [Range; 3],
}

impl Solution {
    pub(super) fn new(ranges: [Range; 3]) -> Self {
        let mut ranges = ranges;
        for idx in 0..ranges.len() {
            // Enforcing the time-order invariant. B can't start before A.
            if idx > 0 && ranges[idx - 1].start > ranges[idx].start {
                ranges[idx].start = ranges[idx - 1].start;
            }

            // And B can't end after C.
            if idx < ranges.len() - 1 && ranges[idx].end > ranges[idx + 1].end {
                ranges[idx].end = ranges[idx + 1].end;
            }
        }
        Self { ranges }
    }

    /// 1. It chooses (from A, B or C) a range to split in half.
    /// 2. It shapes two new Solution instances where the chosen range is split
    ///    in half (1st solution takes the left half, 2nd — the right half) and
    ///    keep the rest two ranges the same.
    /// 3. It returns the two new Solution instances.
    ///
    /// Note: it returns an empty vector if there is nothing to split.
    pub(super) fn branch(&self, range_boxes: &RangeBoxes) -> Vec<Self> {
        // Find a good candidate (from A, B or C) to split in half.
        let idx = self.branch_idx(range_boxes);
        if self.ranges[idx].count() == 1 {
            // It's a leaf. The triangle with it is already calculated. Skip.
            return Vec::new();
        }

        let mut left = self.ranges; // A copy.
        let mut right = self.ranges; // A copy.
        left[idx] = self.ranges[idx].left();
        right[idx] = self.ranges[idx].right();
        vec![Self::new(left), Self::new(right)]
    }

    /// Determines which range (from A, B or C) is better to split in half to
    /// search for a better triangle.
    fn branch_idx(&self, range_boxes: &RangeBoxes) -> usize {
        // Find which range (from A, B, C) has the biggest number of fixes.
        let mut idx = 0;
        for candidate in 1..self.ranges.len() {
            if self.ranges[candidate].count() > self.ranges[idx].count() {
                idx = candidate;
            }
        }

        // Assume that the range with 8x bigger area is better than the range
        // with the biggest number of fixes.
        let mut div_area = range_boxes.query(self.ranges[idx]).area();
        for candidate in 0..self.ranges.len() {
            let candidate_area = range_boxes.query(self.ranges[candidate]).area();
            if self.ranges[candidate].count() > 1 && candidate_area > div_area * 8.0 {
                idx = candidate;
                div_area = candidate_area;
            }
        }
        idx
    }
}
