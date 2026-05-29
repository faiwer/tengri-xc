pub(super) struct ClosurePairs {
}

impl ClosurePairs {
    pub(super) fn new(points: &[Point]) -> Self {
    }

    pub(super) fn closest_pair(
        &self,
        prefix_end: usize,
        suffix_start: usize,
        points: &[Point],
    ) -> Option<CachedClosure> {
        if prefix_end >= points.len() || suffix_start >= points.len() || suffix_start <= prefix_end
        {
            return None;
        }

