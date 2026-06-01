use std::collections::BinaryHeap;

use crate::flight::types::Track;
use crate::geo::METERS_PER_KM;

use super::super::types::{leg_distance_m, to_track_point};
use super::super::{Route, RouteClosure, RouteType, RouteWaypoint, ScoringOutcome};
use super::bounds::max_fai_distance;
use super::closure::ClosurePairs;
use super::geometry::{Point, Range, RangeBoxes};
use super::types::{
    BestSolution, Closure, NodeTraceEvent, PendingSolution, ScoreInfo, Solution, SummaryTraceEvent,
    TraceEvent, TraceEventKind, TriangleOptions, to_trace_ranges,
};

pub(crate) struct FaiTriangleEvaluator<'a> {
    track: &'a Track,
    points: Vec<Point>,
    /// Segment tree of per-range bounding boxes used for branch bounds.
    range_boxes: RangeBoxes,
    /// Cached nearest prefix/suffix pairs for triangle closure checks.
    closure_pairs: ClosurePairs,
    /// Best concrete candidate scored so far.
    best: BestSolution,
    /// Number of non-pruned child states scored after the root candidate.
    processed: u64,
    options: TriangleOptions,
    /// Maximum closure gap as a fraction of candidate perimeter. If the closure
    /// is bigger than this value, the triangle is not valid.
    closing_distance_relative: f64,
    /// For FAI triangles. Perimeter floor implied by `min_scoring_side_km` and
    /// the 28% side rule. It makes no sense to score a triangle with a
    /// perimeter shorter than this value.
    min_scoring_distance_km: Option<f64>,
}

impl<'a> FaiTriangleEvaluator<'a> {
    pub(crate) fn new(track: &'a Track, options: TriangleOptions) -> Self {
        let points = track
            .points
            .iter()
            .map(Point::from_track_point)
            .collect::<Vec<_>>();
        let range_boxes = RangeBoxes::new(&points);
        let closure_pairs = ClosurePairs::new(&points);
        let closing_distance_relative = options.closure_ratio();
        let min_scoring_distance_km = match (options.min_side, options.min_scoring_side_km) {
            // The FAI triangle rules. min_side is 28%, min_scoring_side_km is
            // either 1.4km or dereived from the free-distance result.
            (Some(min_side), Some(min_scoring_side_km)) => Some(min_scoring_side_km / min_side),
            _ => None,
        };
        Self {
            track,
            points,
            range_boxes,
            closure_pairs,
            best: BestSolution::empty(),
            processed: 0,
            options,
            closing_distance_relative,
            min_scoring_distance_km,
        }
    }

    pub(crate) fn new_with_closure(
        track: &'a Track,
        options: TriangleOptions,
        closing_distance_relative: f64,
    ) -> Self {
        let mut evaluator = Self::new(track, options);
        evaluator.closing_distance_relative = closing_distance_relative;
        evaluator
    }

    pub(crate) fn evaluate(
        &mut self,
        mut trace: Option<&mut dyn FnMut(&TraceEvent)>,
    ) -> ScoringOutcome<Route> {
        if self.points.len() < 3 {
            return ScoringOutcome::NoAnswer;
        }

        let end = self.points.len() - 1;
        let root = Solution::new([Range::new(0, end), Range::new(0, end), Range::new(0, end)]);
        let mut next_id = 0;
        let root = PendingSolution {
            bound: self.compute_upper_bound(&root),
            score: 0.0,
            score_info: None,
            id: next_id,
            solution: root,
        };
        self.best = root.to_best();
        let mut pending = BinaryHeap::new();
        pending.push(root);

        while let Some(current) = pending.pop() {
            // Note: we compare the "upper bound" of the current candidate
            // against the best score of a real FAI-triangle found thus far.
            if current.bound <= self.best.score {
                // The heap has only smaller triangles left. No need to continue.
                pending.clear();
                break;
            }

            // Split the current solution into two new sub-solutions and score them.
            for solution in current.solution.branch(&self.range_boxes) {
                let bound = self.compute_upper_bound(&solution);
                if bound <= self.best.score {
                    if let Some(cb) = trace.as_deref_mut() {
                        // Trace the pruned sub-solution for debugging purposes.
                        cb(&TraceEvent::Node(NodeTraceEvent {
                            kind: TraceEventKind::Pruned,
                            processed: self.processed,
                            ranges: to_trace_ranges(&solution),
                            bound,
                            score: None,
                            best: self.best.score,
                            pending: pending.len(),
                        }));
                    }
                    continue; // Skip this sub-solution, it's not better than the best.
                }

                let score_info = self.score_solution_center(&solution);
                self.processed += 1;
                next_id += 1;
                let pending_solution = PendingSolution {
                    bound,
                    score: score_info.map_or(0.0, |score| score.score),
                    score_info,
                    id: next_id,
                    solution,
                };
                if let Some(cb) = trace.as_deref_mut() {
                    cb(&TraceEvent::Node(NodeTraceEvent {
                        kind: TraceEventKind::Kept,
                        processed: self.processed,
                        ranges: to_trace_ranges(&pending_solution.solution),
                        bound: pending_solution.bound,
                        score: Some(pending_solution.score),
                        best: self.best.score,
                        pending: pending.len(),
                    }));
                }

                if pending_solution.score >= self.best.score && pending_solution.score > 0.0 {
                    // Possibly it's the best solution so far.
                    self.best = pending_solution.to_best();
                }

                // Even if it's worse than the best
                pending.push(pending_solution);
            }
        }

        // Thus far, we either found nothing (score = 0) or found the solution.
        // Even though the solution is based on the three range centers, we know
        // that the center fix of each is the right answer fix. The rest are
        // worse or equal.
        if let Some(cb) = trace {
            cb(&TraceEvent::Summary(SummaryTraceEvent {
                processed: self.processed,
                current_upper_bound: self.best.score,
                closure_cache_stats: self.closure_pairs.cache_stats(),
            }));
        }

        self.best_route()
    }

    /// Convert the found best solution to a Route.
    fn best_route(&self) -> ScoringOutcome<Route> {
        let Some(score_info) = self.best.score_info else {
            // No FAI triangle found.
            return ScoringOutcome::NoAnswer;
        };

        // Map indexes to GPS-fixes.
        let turnpoints = score_info
            .turnpoints
            .into_iter()
            .map(|idx| RouteWaypoint::Point {
                fix: self.track.points[idx],
            })
            .collect::<Vec<_>>();
        let leg_distances = fai_triangle_legs_m(
            turnpoints
                .as_slice()
                .try_into()
                .expect("always 3 turnpoints"),
        );
        let raw_distance_m = leg_distances.iter().copied().sum::<u32>();
        let closure = RouteClosure {
            start: RouteWaypoint::Point {
                fix: self.track.points[score_info.closure.start_idx],
            },
            end: RouteWaypoint::Point {
                fix: self.track.points[score_info.closure.end_idx],
            },
            distance: leg_distance_m(
                &self.track.points[score_info.closure.start_idx],
                &self.track.points[score_info.closure.end_idx],
            ),
        };
        let factor = self.options.multiplier;
        let distance_m = raw_distance_m.saturating_sub(closure.distance);
        ScoringOutcome::Answer(Route {
            id: 0, // A stub. Will be filled in by the caller.
            flight_id: "draft".to_owned(),
            route_type: RouteType::FaiTriangle,
            sub_type: self.options.sub_type,
            turnpoints,
            leg_distances,
            distance: distance_m,
            closure: Some(closure),
            score: (distance_m as f64 / METERS_PER_KM) * factor,
            factor,
            optimal: true,
            scored_ms: 0, // A stub. Will be filled in by the caller.
        })
    }

    /// Compute the optimistic upper bound for a given solution.
    fn compute_upper_bound(&self, solution: &Solution) -> f64 {
        let boxes = solution.ranges.map(|range| self.range_boxes.query(range));
        let max_fai_distance = max_fai_distance(
            boxes,
            self.options.min_side,
            self.options.min_scoring_side_km,
        );
        if max_fai_distance == 0.0 {
            return 0.0;
        }

        if solution.ranges[0].end < solution.ranges[2].start {
            // Calculate the shortest possible closure between the two ranges.
            // We take A.end and C.start to consume as many fixes as possible.
            let Some(closure) = self.triangle_closure(
                solution.ranges[0].end,
                solution.ranges[2].start,
                max_fai_distance,
            ) else {
                return 0.0;
            };

            return (max_fai_distance - closure.distance_km) * self.options.multiplier;
        }

        // A & C have overlapping time ranges. But the FAI triangle here is still possible.
        max_fai_distance * self.options.multiplier
    }

    /// Score a triangle by the center of its three ranges. Return `None` if the
    /// triangle is not valid, too small or not a triangle.
    fn score_solution_center(&self, solution: &Solution) -> Option<ScoreInfo> {
        let indexes = solution.ranges.map(Range::center);
        if indexes[0] >= indexes[1] || indexes[1] >= indexes[2] {
            return None; // A can't start after B, and B can't start after C.
        }
        self.score_turnpoints(indexes)
    }

    /// Score a triangle by its center turnpoints. Return `None` if the triangle
    /// is not valid, too small or not a triangle.
    fn score_turnpoints(&self, indexes: [usize; 3]) -> Option<ScoreInfo> {
        // Use "haversine" instead of "fcc" because we use "haversine" at the
        // final step to be consistent with competitors.
        let legs = [
            self.points[indexes[0]].distance_haversine_km(&self.points[indexes[1]]),
            self.points[indexes[1]].distance_haversine_km(&self.points[indexes[2]]),
            self.points[indexes[2]].distance_haversine_km(&self.points[indexes[0]]),
        ];

        if let Some(min_scoring_side_km) = self.options.min_scoring_side_km
            && legs.iter().any(|&leg| leg < min_scoring_side_km)
        {
            // A side is shorter than the minimum scoring side. Bail out,
            // because the triangle is too small to be meaningful.
            return None;
        }

        let distance_km = legs.into_iter().sum::<f64>();
        if let Some(min_scoring_distance_km) = self.min_scoring_distance_km
            && distance_km < min_scoring_distance_km
        {
            // It seems we have a way bigger "Free Distance" than this
            // triangle. Bail out, to avoid wasting CPU.
            return None;
        }

        if let Some(min_side) = self.options.min_side {
            let min_side_km = min_side * distance_km;
            if legs.iter().any(|&leg| leg < min_side_km) {
                return None;
            }
        }

        let closure = self.triangle_closure(indexes[0], indexes[2], distance_km)?;
        let score = (distance_km - closure.distance_km) * self.options.multiplier;
        Some(ScoreInfo {
            closure,
            turnpoints: indexes,
            score,
        })
    }

    /// Find a pair of before-A and after-C fixes that are closest to each other.
    /// Return `None` if the closure is bigger than the allowed maximum.
    fn triangle_closure(
        &self,
        first_tp: usize,
        last_tp: usize,
        distance_km: f64,
    ) -> Option<Closure> {
        let closure = self.closest_pair(first_tp, last_tp);
        (closure.distance_km <= distance_km * self.closing_distance_relative).then_some(closure)
    }

    fn closest_pair(&self, first_tp: usize, last_tp: usize) -> Closure {
        self.closure_pairs
            .closest_pair(first_tp, last_tp, &self.points)
            .map(|closure| Closure {
                start_idx: closure.q_idx,
                end_idx: closure.w_idx,
                distance_km: closure.distance_km,
            })
            .unwrap_or(Closure {
                start_idx: first_tp,
                end_idx: last_tp,
                distance_km: f64::INFINITY,
            })
    }
}

/// Calculate the legs of a FAI triangle based on the three turnpoints.
fn fai_triangle_legs_m([a, b, c]: &[RouteWaypoint; 3]) -> Vec<u32> {
    let [a, b, c] = [to_track_point(a), to_track_point(b), to_track_point(c)];
    vec![
        leg_distance_m(a, b),
        leg_distance_m(b, c),
        leg_distance_m(c, a),
    ]
}
