//! What a route contributes to the picture, filtered like the client's
//! `buildRouteGeometry`. The markers and the legs both read it, so they can't
//! disagree about which fixes count.

use tengri_geo::PointE5;

use crate::flight::{Route, RouteSubType, RouteWaypoint};

pub(super) struct RouteShape {
    /// The client's `isOlcTriangle`: only these subtypes draw a closure.
    pub olc_triangle: bool,
    pub closure_start: Option<PointE5>,
    pub turnpoints: Vec<PointE5>,
    pub closure_end: Option<PointE5>,
}

impl RouteShape {
    pub(super) fn new(route: &Route) -> Self {
        let olc_triangle = matches!(
            route.sub_type,
            RouteSubType::OlcOpen | RouteSubType::OlcClosed
        );
        let closure = olc_triangle.then_some(route.closure.as_ref()).flatten();

        Self {
            olc_triangle,
            closure_start: closure.and_then(|closure| point_fix(&closure.start)),
            turnpoints: route.turnpoints.iter().filter_map(point_fix).collect(),
            closure_end: closure.and_then(|closure| point_fix(&closure.end)),
        }
    }

    /// `[closure start, turnpoints…, closure end]`, deduplicated — the points
    /// that get a marker.
    pub(super) fn waypoints(&self) -> Vec<PointE5> {
        let mut unique: Vec<PointE5> = Vec::new();
        for &point in self
            .closure_start
            .iter()
            .chain(self.turnpoints.iter())
            .chain(self.closure_end.iter())
        {
            if !unique.contains(&point) {
                unique.push(point);
            }
        }
        unique
    }
}

/// Cylinder and line waypoints are dropped, as on the client; nothing produces
/// them yet.
fn point_fix(waypoint: &RouteWaypoint) -> Option<PointE5> {
    match waypoint {
        RouteWaypoint::Point { fix } => Some(PointE5::new(fix.lat, fix.lon)),
        RouteWaypoint::Cylinder { .. } | RouteWaypoint::Line { .. } => None,
    }
}
