use super::{
    HasE5Coords, Point,
    consts::{E5_TO_RAD, EARTH_RADIUS_M},
};

/// Fast approximate surface distance in metres between two E5 lat/lon points.
/// Relatively accurate for short distances at any latitude. Even Alaska.
/// Since it has only 1 cos it's ~6x faster than the Haversine formula.
pub fn approximate_distance_m(lat_a_e5: i32, lon_a_e5: i32, lat_b_e5: i32, lon_b_e5: i32) -> f64 {
    let mid_lat = (lat_a_e5 + lat_b_e5) as f64 * 0.5 * E5_TO_RAD;
    let dlat = (lat_b_e5 - lat_a_e5) as f64 * E5_TO_RAD;
    let dlon = (lon_b_e5 - lon_a_e5) as f64 * E5_TO_RAD * mid_lat.cos();

    (dlat * dlat + dlon * dlon).sqrt() * EARTH_RADIUS_M
}

/// Project E5 points into an approximate local metre plane. Uses per-point
/// `cos(lat)` for longitude scaling — more accurate than a single midpoint
/// scale but still a flat-earth approximation.
pub(crate) fn project_track_points_m<P: HasE5Coords>(points: &[P]) -> Vec<Point> {
    let n = points.len() as f64;
    let mean_lat = points
        .iter()
        .map(|point| point.lat_e5() as f64 * E5_TO_RAD)
        .sum::<f64>()
        / n;
    let mean_lon = points
        .iter()
        .map(|point| point.lon_e5() as f64 * E5_TO_RAD)
        .sum::<f64>()
        / n;

    points
        .iter()
        .map(|point| {
            let lat = point.lat_e5() as f64 * E5_TO_RAD;
            let lon = point.lon_e5() as f64 * E5_TO_RAD;
            Point::new(
                (lon - mean_lon) * lat.cos() * EARTH_RADIUS_M,
                (lat - mean_lat) * EARTH_RADIUS_M,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::flight::types::TrackPoint;

    use super::*;

    fn e5(deg: f64) -> i32 {
        (deg * 1e5).round() as i32
    }

    fn point(lat: f64, lon: f64) -> TrackPoint {
        TrackPoint {
            time: 0,
            lat: e5(lat),
            lon: e5(lon),
            geo_alt: 0,
            pressure_alt: None,
            tas: None,
        }
    }

    #[test]
    fn identical_points_are_zero() {
        let d = approximate_distance_m(e5(47.3769), e5(8.5417), e5(47.3769), e5(8.5417));
        assert_eq!(d, 0.0);
    }

    #[test]
    fn geneva_to_zurich_is_close_enough_for_speed_filtering() {
        let d = approximate_distance_m(e5(46.2044), e5(6.1432), e5(47.3769), e5(8.5417));
        let km = d / 1000.0;
        assert!(
            (223.0..=226.0).contains(&km),
            "Geneva->Zurich expected roughly 224 km, got {km:.3} km"
        );
    }

    #[test]
    fn projection_uses_metres_at_high_latitude() {
        let projected = project_track_points_m(&[point(69.65, 18.95), point(69.65, 18.96)]);
        let dx = (projected[1].x - projected[0].x).abs();
        let dy = (projected[1].y - projected[0].y).abs();

        assert!(
            (380.0..=390.0).contains(&dx),
            "0.01 deg longitude at Tromso latitude should be about 386 m, got {dx:.3} m"
        );
        assert!(
            dy < 1.0,
            "same-latitude projection should have tiny dy, got {dy:.3} m"
        );
    }
}
