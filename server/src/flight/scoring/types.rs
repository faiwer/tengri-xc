use serde::{Deserialize, Serialize};

use crate::flight::types::TrackPoint;
use crate::geo::haversine_m;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "route_type", rename_all = "snake_case")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "route_sub_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RouteSubType {
    None,
    OlcClosed,
    OlcOpen,
    FaiCylinders,
}

pub type RouteFix = [i32; 2];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RouteWaypoint {
    Point {
        fix: TrackPoint,
    },
    Cylinder {
        center: RouteFix,
        mode: Option<RouteCylinderMode>,
        radius: u32,
        tangents: Vec<RouteFix>,
        track_fix: TrackPoint,
    },
    Line {
        track_fix: TrackPoint,
        projection: [RouteFix; 2],
        tangent: RouteFix,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedTrackPoint {
    pub track_idx: usize,
    pub point: TrackPoint,
}

pub(super) fn to_track_point(point: &RouteWaypoint) -> &TrackPoint {
    match point {
        RouteWaypoint::Point { fix } => fix,
        RouteWaypoint::Cylinder { .. } | RouteWaypoint::Line { .. } => {
            panic!("route leg distance expects point waypoints")
        }
    }
}

pub(super) fn leg_distance_m(from: &TrackPoint, to: &TrackPoint) -> u32 {
    haversine_m(from.lat, from.lon, to.lat, to.lon).round() as u32
}
