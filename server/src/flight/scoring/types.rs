/// Route families we evaluate independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    FreeDistance,
    FreeTriangle,
    FaiTriangle,
    ClosedFreeTriangle,
    ClosedFaiTriangle,
}

impl RouteKind {
    pub const ALL: [Self; 5] = [
        Self::FreeDistance,
        Self::FreeTriangle,
        Self::FaiTriangle,
        Self::ClosedFreeTriangle,
        Self::ClosedFaiTriangle,
    ];
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteEvaluation {
    pub routes: Vec<RouteResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteResult {
    pub kind: RouteKind,
    pub distance_m: u32,
    pub points: f64,
    pub turnpoints: Vec<RoutePoint>,
    pub optimal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutePoint {
    pub track_idx: usize,
    pub time: u32,
    pub lat: i32,
    pub lon: i32,
}
