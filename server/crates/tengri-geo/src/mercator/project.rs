//! Absolute Web Mercator (EPSG:3857) metres — the frame slippy-map tiles are
//! cut in.

use crate::{E5_TO_DEGREES, HasE5Coords, Point};

/// WGS84 equatorial radius. Web Mercator is defined on this sphere; the
/// crate's distance helpers use the IUGG mean radius instead, which is a
/// different number on purpose.
pub const WEB_MERCATOR_RADIUS_M: f64 = 6_378_137.0;

/// Side of the square Mercator world, in metres. `x` and `y` both run
/// `-WORLD_SIZE_M / 2 ..= WORLD_SIZE_M / 2`.
pub const WORLD_SIZE_M: f64 = 2.0 * std::f64::consts::PI * WEB_MERCATOR_RADIUS_M;

/// Where the square world stops. Beyond it `y` runs to infinity.
const MAX_LAT_DEG: f64 = 85.051_128_779_806_59;

/// A box in Mercator metres. `min_y` is the southern edge, so it maps to the
/// *bottom* of a north-up canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MercatorRect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl MercatorRect {
    pub fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

pub fn project_mercator_m<P: HasE5Coords + ?Sized>(point: &P) -> Point {
    let lon = point.lon_e5() as f64 * E5_TO_DEGREES;
    let lat = (point.lat_e5() as f64 * E5_TO_DEGREES).clamp(-MAX_LAT_DEG, MAX_LAT_DEG);
    Point::new(
        lon.to_radians() * WEB_MERCATOR_RADIUS_M,
        lat.to_radians().tan().asinh() * WEB_MERCATOR_RADIUS_M,
    )
}

pub fn project_points_mercator_m<P: HasE5Coords>(points: &[P]) -> Vec<Point> {
    points.iter().map(project_mercator_m).collect()
}

/// The box `points` fit in. Cheaper than projecting them all: Mercator's `x`
/// depends only on longitude and its `y` only on latitude, so the corners of
/// the E5 bounding box project to the corners of the Mercator one.
pub fn mercator_bounds<P: HasE5Coords>(points: &[P]) -> Option<MercatorRect> {
    let (mut min_lat, mut max_lat) = (i32::MAX, i32::MIN);
    let (mut min_lon, mut max_lon) = (i32::MAX, i32::MIN);
    for point in points {
        min_lat = min_lat.min(point.lat_e5());
        max_lat = max_lat.max(point.lat_e5());
        min_lon = min_lon.min(point.lon_e5());
        max_lon = max_lon.max(point.lon_e5());
    }
    if points.is_empty() {
        return None;
    }

    let south_west = project_mercator_m(&crate::PointE5::new(min_lat, min_lon));
    let north_east = project_mercator_m(&crate::PointE5::new(max_lat, max_lon));
    Some(MercatorRect::new(
        south_west.x,
        south_west.y,
        north_east.x,
        north_east.y,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PointE5;

    #[test]
    fn the_null_island_is_the_origin() {
        let origin = project_mercator_m(&PointE5::new(0, 0));

        assert_eq!(origin.x, 0.0);
        assert_eq!(origin.y, 0.0);
    }

    #[test]
    fn the_date_line_sits_on_the_world_edge() {
        let east = project_mercator_m(&PointE5::new(0, 180_00000));

        assert!((east.x - WORLD_SIZE_M / 2.0).abs() < 1e-6, "{}", east.x);
    }

    #[test]
    fn north_is_positive_y() {
        let alps = project_mercator_m(&PointE5::new(45_30000, 6_05000));

        assert!(alps.y > 0.0);
        assert!(alps.x > 0.0);
    }

    #[test]
    fn a_pole_clamps_instead_of_running_to_infinity() {
        let pole = project_mercator_m(&PointE5::new(90_00000, 0));

        assert!((pole.y - WORLD_SIZE_M / 2.0).abs() < 1.0, "{}", pole.y);
    }

    #[test]
    fn the_bounds_bracket_every_point() {
        let points = [
            PointE5::new(45_30000, 6_05000),
            PointE5::new(46_10000, 5_80000),
            PointE5::new(45_70000, 6_40000),
        ];

        let bounds = mercator_bounds(&points).unwrap();

        for projected in project_points_mercator_m(&points) {
            assert!(projected.x >= bounds.min_x && projected.x <= bounds.max_x);
            assert!(projected.y >= bounds.min_y && projected.y <= bounds.max_y);
        }
    }

    #[test]
    fn an_empty_track_has_no_bounds() {
        assert!(mercator_bounds::<PointE5>(&[]).is_none());
    }
}
