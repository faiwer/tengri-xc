use tengri_geo::PointE5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoringTrack {
    pub points: Vec<PointE5>,
}

impl ScoringTrack {
    pub fn select_at<I>(&self, indexes: I) -> Self
    where
        I: IntoIterator<Item = usize>,
    {
        Self {
            points: indexes.into_iter().map(|idx| self.points[idx]).collect(),
        }
    }
}
