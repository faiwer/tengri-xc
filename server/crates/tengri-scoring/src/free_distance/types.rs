use super::super::RoutePoint;

#[derive(Debug, Clone)]
pub(super) struct FreeDistanceScore {
    pub(super) turnpoints: Vec<RoutePoint>,
}
