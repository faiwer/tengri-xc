//! Renders a flight into a small JPEG for link previews: the track as a
//! vario-coloured polyline, with the scored route's legs and waypoints drawn
//! over it like the client's `TrackRoute`, optionally over satellite imagery.
//! Persisting the render is still to come.

use anyhow::anyhow;
use tengri_formats::{Track, find_flight_window};
use tengri_geo::{PointE5, mercator_bounds, project_points_mercator_m};

use crate::{config::SatelliteMap, flight::Route};

mod attribution;
mod basemap;
mod gate;
mod jpeg;
mod layout;
mod legs;
mod paint;
mod route_shape;
mod track;
mod vario;
mod waypoints;

#[cfg(test)]
mod fixtures;

pub use basemap::Basemap;
pub use gate::{RenderGate, RenderLease, Turn};

/// The track's polyline, with the route's legs and waypoints on top — or just
/// takeoff and landing when the flight hasn't been scored.
pub fn render_flight_image(
    flight: &Track,
    route: Option<&Route>,
    basemap: Option<&Basemap>,
) -> anyhow::Result<Vec<u8>> {
    let Some(bounds) = mercator_bounds(&flight.points) else {
        return Err(anyhow!("track has no points"));
    };

    let window = find_flight_window(flight);
    let fixes = waypoints::fixes(flight, route, window);
    // An unscored flight has two lone markers and nothing to join them with.
    let legs = route
        .map(|route| legs::legs(route, &fixes))
        .unwrap_or_default();

    let mut all: Vec<PointE5> = flight.points.iter().map(PointE5::from_e5_coords).collect();
    all.extend(fixes);
    let projected = project_points_mercator_m(&all);
    let (track_points, waypoints_points) = projected.split_at(flight.points.len());

    let layout = layout::Layout::new(bounds);
    let mut pixmap = layout.canvas()?;
    if let Some(basemap) = basemap {
        basemap::draw_basemap(&mut pixmap, &layout, basemap);
        if let Some(credit) = basemap.attribution() {
            attribution::draw_attribution(&mut pixmap, credit);
        }
    }
    track::draw_track(
        &mut pixmap,
        &layout,
        track_points,
        &vario::runs(flight, window),
    );
    legs::draw_legs(&mut pixmap, &layout, waypoints_points, &legs);
    waypoints::draw_waypoints(&mut pixmap, &layout, waypoints_points);

    jpeg::encode(&pixmap)
}

/// The satellite backdrop for `flight`. `None` when the tiles don't all arrive
/// in time — the render then falls back to a white canvas.
///
/// Split out of [`render_flight_image`] because it's the only part that
/// awaits; the render itself stays a blocking call.
pub async fn fetch_basemap(satellite: &SatelliteMap, flight: &Track) -> Option<Basemap> {
    let layout = layout::Layout::new(mercator_bounds(&flight.points)?);
    basemap::fetch(satellite, &layout).await
}

#[cfg(test)]
mod tests {
    use super::fixtures::{sample_track, triangle};
    use super::*;
    use crate::flight::RouteSubType;

    #[test]
    fn renders_a_jpeg_with_and_without_a_route() {
        let track = sample_track();

        for route in [None, Some(triangle(RouteSubType::None, None))] {
            let jpeg = render_flight_image(&track, route.as_ref(), None).unwrap();

            assert_eq!(&jpeg[..2], &[0xff, 0xd8], "SOI marker");
            assert_eq!(&jpeg[jpeg.len() - 2..], &[0xff, 0xd9], "EOI marker");
        }
    }

    #[test]
    fn the_route_rides_in_the_track_s_frame() {
        let track = sample_track();
        let route = triangle(RouteSubType::None, None);

        let with_route = render_flight_image(&track, Some(&route), None).unwrap();
        let takeoff_and_landing = render_flight_image(&track, None, None).unwrap();

        assert_ne!(
            with_route, takeoff_and_landing,
            "the waypoints should land on the canvas, not off it"
        );
    }

    #[test]
    fn an_empty_track_is_an_error() {
        let track = Track {
            start_time: 0,
            points: Vec::new(),
        };

        assert!(render_flight_image(&track, None, None).is_err());
    }
}
