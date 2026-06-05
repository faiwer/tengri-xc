use serde::{Deserialize, Serialize};
use tengri_geo::{PointE5, haversine_m};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Route {
    pub id: i64,
    pub flight_id: String,
    pub route_type: RouteType,
    pub sub_type: RouteSubType,
    pub turnpoints: Vec<RouteWaypoint>,
    pub leg_distances: Vec<u32>,
    pub distance: u32,
    pub score: f64,
    pub factor: f64,
    pub optimal: bool,
    pub closure: Option<RouteClosure>,
    /// Wall-clock milliseconds spent evaluating this route type.
    pub scored_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "route_type", rename_all = "snake_case")
)]
#[serde(rename_all = "snake_case")]
pub enum RouteType {
    FreeDistance,
    FaiTriangle,
    FreeTriangle,
    Task,
}

impl RouteType {
    pub const SCORABLE: [Self; 3] = [Self::FreeDistance, Self::FreeTriangle, Self::FaiTriangle];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "route_sub_type", rename_all = "snake_case")
)]
#[serde(rename_all = "snake_case")]
pub enum RouteSubType {
    None,
    OlcClosed,
    OlcOpen,
    FaiCylinders,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutePoint {
    pub idx: usize,
    pub lat: i32,
    pub lon: i32,
}

impl RoutePoint {
    pub(super) fn from(idx: usize, point: PointE5) -> Self {
        Self {
            idx,
            lat: point.lat,
            lon: point.lon,
        }
    }

    pub(super) fn from_waypoint(point: &RouteWaypoint) -> &Self {
        match point {
            RouteWaypoint::Point { fix } => fix,
            RouteWaypoint::Cylinder { track_fix, .. } | RouteWaypoint::Line { track_fix, .. } => {
                track_fix
            }
        }
    }
}

pub type RouteFix = [i32; 2];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RouteWaypoint {
    Point {
        fix: RoutePoint,
    },
    Cylinder {
        center: RouteFix,
        mode: Option<RouteCylinderMode>,
        radius: u32,
        tangents: Vec<RouteFix>,
        track_fix: RoutePoint,
    },
    Line {
        track_fix: RoutePoint,
        projection: [RouteFix; 2],
        tangent: RouteFix,
    },
}

impl RouteWaypoint {
    pub(super) fn from_route_point(fix: RoutePoint) -> Self {
        Self::Point { fix }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteCylinderMode {
    Enter,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteClosure {
    pub start: RouteWaypoint,
    pub end: RouteWaypoint,
    pub distance: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteEvaluation {
    pub routes: Vec<ScoringOutcome<Route>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScoringOutcome<T> {
    Answer(T),
    NoAnswer,
    Error(ScoringError),
}

impl<T> ScoringOutcome<T> {
    pub fn map_answer<U>(self, map: impl FnOnce(T) -> U) -> ScoringOutcome<U> {
        match self {
            Self::Answer(value) => ScoringOutcome::Answer(map(value)),
            Self::NoAnswer => ScoringOutcome::NoAnswer,
            Self::Error(error) => ScoringOutcome::Error(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScoringError {
    #[error("{route_type:?} scorer failed: {reason}")]
    SolverFailed {
        route_type: RouteType,
        reason: &'static str,
    },
}

pub(super) fn leg_distance_m(from: &RoutePoint, to: &RoutePoint) -> u32 {
    haversine_m(from.lat, from.lon, to.lat, to.lon).round() as u32
}
