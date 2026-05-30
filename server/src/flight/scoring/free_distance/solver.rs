use super::super::shared::Point;

/// Number of points in the valid free-distance route.
const ROUTE_POINTS: usize = 5;

/// Keeps only the fixes around the previous route's turnpoints so the next
/// refinement pass works on a much smaller track. The window around each
/// turnpoint is `percentage`% of `point_count` wide (≥ 1 fix), wide enough that
/// the true optimum stays inside it.
pub(super) fn squeeze_route(
    point_count: usize,
    route_indexes: &[usize],
    percentage: f64,
) -> Vec<usize> {
    let radius = ((point_count as f64 * percentage / 100.0).ceil() as usize).max(1);
    let mut keep = vec![false; point_count];
    for &idx in route_indexes {
        let start = idx.saturating_sub(radius);
        let end = (idx + radius).min(point_count - 1);
        for keep in &mut keep[start..=end] {
            *keep = true;
        }
    }

    keep.into_iter()
        .enumerate()
        .filter_map(|(idx, keep)| keep.then_some(idx))
        .collect()
}

/// Find the best five-point free-distance route inside `seed_points`.
///
/// The route is exact for the provided seed set. Dynamic programming keeps the
/// best route ending at each seed point for every leg count, then walks the
/// predecessor links back from the best finish.
pub(super) fn find_best_free_distance_dp(
    points: &[Point],
    track: &[usize],
) -> Option<[usize; ROUTE_POINTS]> {
    if track.len() < ROUTE_POINTS {
        return None;
    }

    let n = track.len();
    let mut cache: [Vec<CacheState>; ROUTE_POINTS] =
        std::array::from_fn(|_| vec![CacheState::default(); n]);
    for idx in 0..n {
        cache[0][idx] = CacheState {
            distance_m: 0.0,
            prev: None,
        };
    }

    for leg in 1..ROUTE_POINTS {
        for end in leg..n {
            let mut best = CacheState::default();
            for start in (leg - 1)..end {
                let candidate = cache[leg - 1][start].distance_m
                    + points[track[start]].distance_fcc_m(&points[track[end]]);
                if candidate > best.distance_m {
                    best = CacheState {
                        distance_m: candidate,
                        prev: Some(start),
                    };
                }
            }
            cache[leg][end] = best;
        }
    }

    // The cache is filled. Select the biggest distance.
    let (finish, best) = cache[4]
        .iter()
        .copied()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.distance_m.total_cmp(&b.distance_m))?;
    if best.distance_m == 0.0 {
        return None;
    }

    let mut indexes = [0; 5];
    let mut leg = 4;
    let mut idx = finish;
    indexes[leg] = track[idx];
    while leg > 0 {
        idx = cache[leg][idx]
            .prev
            .expect("best sampled free-distance state should keep its predecessor");
        leg -= 1;
        indexes[leg] = track[idx];
    }
    Some(indexes)
}

#[derive(Debug, Clone, Copy)]
struct CacheState {
    distance_m: f64,
    prev: Option<usize>,
}

impl Default for CacheState {
    /// Start with an unreachable DP state.
    ///
    /// Valid states overwrite this with a real distance and, except for the
    /// first route point, a predecessor link.
    fn default() -> Self {
        Self {
            distance_m: f64::NEG_INFINITY,
            prev: None,
        }
    }
}
