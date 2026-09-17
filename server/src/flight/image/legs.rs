//! The lines between the route's points, styled like the client's
//! `tengri-route-legs` layers: scored legs solid green, a triangle's closing
//! leg dashed, and the flown-but-unscored ends amber.

use tengri_geo::{Point, PointE5};
use tiny_skia::{PathBuilder, Pixmap, Stroke, StrokeDash, Transform};

use super::{
    layout::Layout,
    paint::{Rgb, SCORED, UNSCORED, solid_brush},
    route_shape::RouteShape,
};
use crate::flight::Route;

/// The client's `line-width: 3`.
const WIDTH_PX: f32 = 3.0;
/// The client's `line-dasharray: [2, 2]`, whose units are line widths.
const DASH_PX: f32 = 2.0 * WIDTH_PX;

/// Endpoints are indexes into the same fixes the markers are drawn from, so a
/// leg can't land anywhere a marker isn't.
pub(super) struct Leg {
    from: usize,
    to: usize,
    dashed: bool,
    color: Rgb,
}

/// `fixes` is what [`super::waypoints::fixes`] returned for this route.
pub(super) fn legs(route: &Route, fixes: &[PointE5]) -> Vec<Leg> {
    let shape = RouteShape::new(route);
    let mut legs = Vec::new();
    let mut connect = |from: PointE5, to: PointE5, dashed: bool, color: Rgb| {
        let (Some(from), Some(to)) = (index_of(fixes, from), index_of(fixes, to)) else {
            return;
        };
        // Two names for one point — the closure endpoint that is also the first
        // turnpoint, say. There's no leg to draw.
        if from != to {
            legs.push(Leg {
                from,
                to,
                dashed,
                color,
            });
        }
    };

    for leg in shape.turnpoints.windows(2) {
        connect(leg[0], leg[1], false, SCORED);
    }
    if shape.olc_triangle && shape.turnpoints.len() >= 3 {
        connect(
            shape.turnpoints[shape.turnpoints.len() - 1],
            shape.turnpoints[0],
            true,
            SCORED,
        );
    }
    if let (Some(start), Some(&first)) = (shape.closure_start, shape.turnpoints.first()) {
        connect(start, first, false, UNSCORED);
    }
    if let (Some(&last), Some(end)) = (shape.turnpoints.last(), shape.closure_end) {
        connect(last, end, false, UNSCORED);
    }
    if let (Some(start), Some(end)) = (shape.closure_start, shape.closure_end) {
        connect(start, end, true, UNSCORED);
    }
    legs
}

pub(super) fn draw_legs(pixmap: &mut Pixmap, layout: &Layout, points: &[Point], legs: &[Leg]) {
    for leg in legs {
        let (Some(from), Some(to)) = (points.get(leg.from), points.get(leg.to)) else {
            continue;
        };

        let mut path = PathBuilder::new();
        let (x, y) = layout.to_canvas(*from);
        path.move_to(x, y);
        let (x, y) = layout.to_canvas(*to);
        path.line_to(x, y);
        let Some(path) = path.finish() else {
            continue;
        };

        let stroke = Stroke {
            width: WIDTH_PX,
            dash: leg
                .dashed
                .then(|| StrokeDash::new(vec![DASH_PX, DASH_PX], 0.0))
                .flatten(),
            ..Stroke::default()
        };
        pixmap.stroke_path(
            &path,
            &solid_brush(leg.color),
            &stroke,
            Transform::identity(),
            None,
        );
    }
}

fn index_of(fixes: &[PointE5], point: PointE5) -> Option<usize> {
    fixes.iter().position(|fix| *fix == point)
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{triangle, waypoint};
    use super::super::waypoints;
    use super::*;
    use crate::flight::{RouteClosure, RouteSubType};

    fn route_legs(route: &Route) -> Vec<Leg> {
        let fixes = waypoints::fixes(&super::super::fixtures::sample_track(), Some(route), None);
        legs(route, &fixes)
    }

    fn closure(start: (i32, i32), end: (i32, i32)) -> RouteClosure {
        RouteClosure {
            start: waypoint(start.0, start.1),
            end: waypoint(end.0, end.1),
            distance: 5_000,
        }
    }

    #[test]
    fn turnpoints_are_joined_by_solid_scored_legs() {
        let legs = route_legs(&triangle(RouteSubType::None, None));

        assert_eq!(legs.len(), 2);
        assert!(legs.iter().all(|leg| !leg.dashed && leg.color == SCORED));
        assert_eq!((legs[0].from, legs[0].to), (0, 1));
        assert_eq!((legs[1].from, legs[1].to), (1, 2));
    }

    #[test]
    fn an_olc_triangle_closes_itself_with_a_dashed_leg() {
        let route = triangle(RouteSubType::OlcClosed, None);

        let legs = route_legs(&route);

        let closing = legs.last().unwrap();
        assert!(closing.dashed);
        assert_eq!(closing.color, SCORED);
        assert_eq!((closing.from, closing.to), (2, 0));
    }

    #[test]
    fn the_closure_adds_amber_legs_at_both_ends() {
        let route = triangle(
            RouteSubType::OlcOpen,
            Some(closure((42_00000, 74_00000), (42_10000, 74_10000))),
        );

        let legs = route_legs(&route);

        let amber: Vec<&Leg> = legs.iter().filter(|leg| leg.color == UNSCORED).collect();
        // Closure start to the first turnpoint, the last turnpoint to the
        // closure end, and the closure chord itself.
        assert_eq!(amber.len(), 3);
        assert_eq!(amber.iter().filter(|leg| leg.dashed).count(), 1);
    }

    #[test]
    fn a_non_olc_route_draws_no_closure_legs() {
        let route = triangle(
            RouteSubType::FaiCylinders,
            Some(closure((42_00000, 74_00000), (42_10000, 74_10000))),
        );

        let legs = route_legs(&route);

        assert_eq!(legs.len(), 2);
        assert!(legs.iter().all(|leg| leg.color == SCORED));
    }

    #[test]
    fn a_closure_endpoint_on_a_turnpoint_draws_no_zero_length_leg() {
        // The closure starts on the first turnpoint of `triangle`.
        let route = triangle(
            RouteSubType::OlcOpen,
            Some(closure((42_50000, 74_00000), (42_10000, 74_10000))),
        );

        let legs = route_legs(&route);

        assert!(legs.iter().all(|leg| leg.from != leg.to));
        // Two scored legs, the dashed closing leg, the last turnpoint to the
        // closure end, and the closure chord.
        assert_eq!(legs.len(), 5);
    }
}
