//! The points the flight is remembered by: the scored route's turnpoints and
//! closure, or takeoff and landing when nothing has scored yet. The track
//! itself is drawn end to end, ground segments included; only the markers stop
//! at the flight window.

use tengri_formats::{FlightWindow, Track, TrackPoint};
use tengri_geo::{Point, PointE5};
use tiny_skia::{FillRule, PathBuilder, Pixmap, Stroke, Transform};

use super::{
    layout::Layout,
    paint::{Rgb, WHITE, solid_brush},
};
use crate::flight::{Route, RouteSubType, RouteWaypoint};

/// Matches the client's 10×10 marker with its 2 px border.
const RADIUS_PX: f32 = 5.0;
const BORDER_PX: f32 = 2.0;
/// The client's `SCORED_COLOR`.
const BORDER_COLOR: Rgb = (0x65, 0xc8, 0x32);

/// `[closure start, turnpoints…, closure end]`, deduplicated — mirroring
/// `buildRouteGeometry.tsx`. An unscored flight marks its takeoff and landing
/// instead, so the preview always says where the pilot started and stopped.
pub(super) fn fixes(
    track: &Track,
    route: Option<&Route>,
    window: Option<FlightWindow>,
) -> Vec<PointE5> {
    let mut fixes: Vec<PointE5> = Vec::new();
    let mut push = |point: PointE5| {
        if !fixes.contains(&point) {
            fixes.push(point);
        }
    };

    match route {
        Some(route) => {
            for waypoint in ordered_waypoints(route) {
                // Cylinder and line waypoints are dropped, as on the client;
                // nothing produces them yet.
                if let RouteWaypoint::Point { fix } = waypoint {
                    push(PointE5::new(fix.lat, fix.lon));
                }
            }
        }
        None => {
            for fix in airborne_ends(track, window).into_iter().flatten() {
                push(PointE5::new(fix.lat, fix.lon));
            }
        }
    }
    fixes
}

pub(super) fn draw_waypoints(pixmap: &mut Pixmap, layout: &Layout, points: &[Point]) {
    let fill = solid_brush(WHITE);
    let border = solid_brush(BORDER_COLOR);
    let stroke = Stroke {
        width: BORDER_PX,
        ..Stroke::default()
    };
    // The client's border sits inside the 10 px box, so the ring is centred
    // half a border width in from the edge.
    let ring_radius = RADIUS_PX - BORDER_PX / 2.0;

    for point in points {
        let (x, y) = layout.to_canvas(*point);
        if let Some(disc) = PathBuilder::from_circle(x, y, RADIUS_PX) {
            pixmap.fill_path(&disc, &fill, FillRule::Winding, Transform::identity(), None);
        }
        if let Some(ring) = PathBuilder::from_circle(x, y, ring_radius) {
            pixmap.stroke_path(&ring, &border, &stroke, Transform::identity(), None);
        }
    }
}

/// Takeoff and landing, not the ends of the file — the stored track keeps the
/// hike up and the drive home, and neither deserves a marker. Tracks with no
/// detectable window (all stationary) fall back to the file's ends.
fn airborne_ends(track: &Track, window: Option<FlightWindow>) -> [Option<&TrackPoint>; 2] {
    match window {
        Some(window) => [
            track.points.get(window.takeoff_idx),
            track.points.get(window.landing_idx),
        ],
        None => [track.points.first(), track.points.last()],
    }
}

fn ordered_waypoints(route: &Route) -> impl Iterator<Item = &RouteWaypoint> {
    let closure = matches!(
        route.sub_type,
        RouteSubType::OlcOpen | RouteSubType::OlcClosed
    )
    .then_some(route.closure.as_ref())
    .flatten();

    closure
        .map(|closure| &closure.start)
        .into_iter()
        .chain(route.turnpoints.iter())
        .chain(closure.map(|closure| &closure.end))
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{sample_track, triangle, waypoint};
    use super::*;
    use crate::flight::RouteClosure;
    use tengri_formats::find_flight_window;

    fn unscored(track: &Track) -> Vec<PointE5> {
        fixes(track, None, find_flight_window(track))
    }

    #[test]
    fn olc_routes_bracket_the_turnpoints_with_the_closure() {
        let closure = RouteClosure {
            start: waypoint(42_00000, 74_00000),
            end: waypoint(42_10000, 74_10000),
            distance: 5_000,
        };
        let route = triangle(RouteSubType::OlcOpen, Some(closure));

        let marked = fixes(&sample_track(), Some(&route), None);

        assert_eq!(marked.len(), 5);
        assert_eq!(marked[0].lat, 42_00000);
        assert_eq!(marked[4].lat, 42_10000);
    }

    #[test]
    fn a_non_olc_route_ignores_its_closure() {
        let closure = RouteClosure {
            start: waypoint(42_00000, 74_00000),
            end: waypoint(42_10000, 74_10000),
            distance: 5_000,
        };
        let route = triangle(RouteSubType::FaiCylinders, Some(closure));

        assert_eq!(fixes(&sample_track(), Some(&route), None).len(), 3);
    }

    #[test]
    fn a_closure_endpoint_on_a_turnpoint_is_kept_once() {
        let closure = RouteClosure {
            start: waypoint(42_50000, 74_00000),
            end: waypoint(42_10000, 74_10000),
            distance: 5_000,
        };
        let route = triangle(RouteSubType::OlcClosed, Some(closure));

        assert_eq!(fixes(&sample_track(), Some(&route), None).len(), 4);
    }

    #[test]
    fn an_unscored_flight_marks_takeoff_and_landing() {
        let track = sample_track();

        let marked = unscored(&track);

        assert_eq!(marked.len(), 2);
        assert_eq!(marked[0].lat, track.points[0].lat);
        assert_eq!(marked[1].lat, track.points.last().unwrap().lat);
    }

    #[test]
    fn the_walk_up_and_the_pack_up_get_no_marker() {
        let track = track_with_ground_segments();

        let marked = unscored(&track);

        // Fix 400 leaves the ground; fix 1000 is the first one back on it.
        assert_eq!(marked.len(), 2);
        assert_eq!(marked[0].lat, track.points[400].lat);
        assert_eq!(marked[1].lat, track.points[1_000].lat);
    }

    /// 400 s walking to launch, 600 s airborne, 400 s packing up — both ground
    /// segments outlast the detector's landing threshold.
    fn track_with_ground_segments() -> Track {
        let fix = |time: u32, lat: i32, lon: i32| TrackPoint {
            time,
            lat,
            lon,
            geo_alt: 10_000,
            pressure_alt: None,
            tas: None,
        };

        // ~1 m/s on the ground, ~50 m/s in the air.
        let mut points: Vec<TrackPoint> = (0..400)
            .map(|i| fix(i as u32, 42_40000 + i, 74_00000))
            .collect();
        points.extend((0..600).map(|i| {
            fix(
                400 + i as u32,
                42_40400 + (i + 1) * 50,
                74_00000 + (i + 1) * 83,
            )
        }));
        let landing = *points.last().unwrap();
        points.extend((0..400).map(|i| fix(1_000 + i as u32, landing.lat + i + 1, landing.lon)));

        Track {
            start_time: 0,
            points,
        }
    }

    #[test]
    fn a_flight_that_landed_where_it_started_marks_one_point() {
        let mut track = sample_track();
        let takeoff = track.points[0];
        track.points.push(takeoff);

        assert_eq!(unscored(&track).len(), 1);
    }
}
