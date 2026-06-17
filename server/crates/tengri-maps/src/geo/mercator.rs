//! Forward/inverse Web Mercator (EPSG:3857) math.
//!
//! Used for translating a Mercator GeoTIFF's projected-metre coordinates
//! to/from lat/lon. The XYZ tile bounds in [`super::xyz`] do the same math in
//! normalised coordinates.

/// WGS-84 semi-major axis in metres. The web-Mercator standard treats the Earth
/// as a sphere of this radius; do not substitute the smaller spherical radius
/// (`6_371_000 m`) used elsewhere in the project for distance math.
pub const WEB_MERCATOR_RADIUS_M: f64 = 6_378_137.0;

/// Half of the projected x extent of the world in EPSG:3857 metres (`π ·
/// WEB_MERCATOR_RADIUS_M`). The full world spans
/// `[-WEB_MERCATOR_HALF_EQUATOR_M, +WEB_MERCATOR_HALF_EQUATOR_M]` in both x and
/// y.
pub const WEB_MERCATOR_HALF_EQUATOR_M: f64 = std::f64::consts::PI * WEB_MERCATOR_RADIUS_M;

/// Project a longitude in degrees onto Web Mercator x-metres.
pub fn lon_to_mercator_x_m(lon_deg: f64) -> f64 {
    lon_deg.to_radians() * WEB_MERCATOR_RADIUS_M
}

/// Inverse of [`lon_to_mercator_x_m`]. Clamped to `[-180, 180]` so a caller
/// passing exactly `±WEB_MERCATOR_HALF_EQUATOR_M` doesn't get the
/// `180.000…0003` ULP overshoot that `to_degrees` produces from `π`.
pub fn mercator_x_m_to_lon(x_m: f64) -> f64 {
    (x_m / WEB_MERCATOR_RADIUS_M)
        .to_degrees()
        .clamp(-180.0, 180.0)
}

/// Project a latitude in degrees onto Web Mercator y-metres.
///
/// Saturates near the poles where the projection diverges. Past
/// `±WEB_MERCATOR_MAX_LAT` the value is `±WEB_MERCATOR_HALF_EQUATOR_M`. The
/// result is also clamped to the world's metric extent so a caller passing
/// exactly `±WEB_MERCATOR_MAX_LAT` doesn't get an ULP-level overshoot of the
/// world edge.
pub fn lat_to_mercator_y_m(lat_deg: f64) -> f64 {
    let clamped = lat_deg.clamp(-super::WEB_MERCATOR_MAX_LAT, super::WEB_MERCATOR_MAX_LAT);
    let y_m = WEB_MERCATOR_RADIUS_M * clamped.to_radians().tan().asinh();
    y_m.clamp(-WEB_MERCATOR_HALF_EQUATOR_M, WEB_MERCATOR_HALF_EQUATOR_M)
}

/// Inverse of [`lat_to_mercator_y_m`]. Clamped to `±WEB_MERCATOR_MAX_LAT` so a
/// caller passing the world-edge metric value doesn't get an ULP-level
/// overshoot of the geographic edge.
pub fn mercator_y_m_to_lat(y_m: f64) -> f64 {
    (y_m / WEB_MERCATOR_RADIUS_M)
        .sinh()
        .atan()
        .to_degrees()
        .clamp(-super::WEB_MERCATOR_MAX_LAT, super::WEB_MERCATOR_MAX_LAT)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON_M: f64 = 1e-6;
    const EPSILON_DEG: f64 = 1e-9;

    #[test]
    fn equator_projects_to_zero() {
        assert!(lat_to_mercator_y_m(0.0).abs() < EPSILON_M);
        assert!(mercator_y_m_to_lat(0.0).abs() < EPSILON_DEG);
    }

    #[test]
    fn prime_meridian_projects_to_zero() {
        assert!(lon_to_mercator_x_m(0.0).abs() < EPSILON_M);
        assert!(mercator_x_m_to_lon(0.0).abs() < EPSILON_DEG);
    }

    #[test]
    fn antimeridian_lands_at_half_equator() {
        let x = lon_to_mercator_x_m(180.0);
        assert!((x - WEB_MERCATOR_HALF_EQUATOR_M).abs() < EPSILON_M);
        assert!((mercator_x_m_to_lon(x) - 180.0).abs() < EPSILON_DEG);
    }

    #[test]
    fn web_mercator_max_lat_lands_at_half_equator() {
        let y = lat_to_mercator_y_m(super::super::WEB_MERCATOR_MAX_LAT);
        assert!((y - WEB_MERCATOR_HALF_EQUATOR_M).abs() < 1e-3);
    }

    #[test]
    fn lat_round_trips_within_an_arcsecond() {
        for lat in [-84.0, -60.0, -23.5, 0.0, 12.345, 51.5, 78.9, 84.999] {
            let projected = lat_to_mercator_y_m(lat);
            let recovered = mercator_y_m_to_lat(projected);
            assert!(
                (recovered - lat).abs() < EPSILON_DEG,
                "lat {lat}: round-tripped to {recovered}",
            );
        }
    }

    #[test]
    fn beyond_max_lat_clamps_to_pole() {
        let above = lat_to_mercator_y_m(super::super::WEB_MERCATOR_MAX_LAT + 1.0);
        assert!((above - WEB_MERCATOR_HALF_EQUATOR_M).abs() < 1e-3);

        let below = lat_to_mercator_y_m(-super::super::WEB_MERCATOR_MAX_LAT - 1.0);
        assert!((below + WEB_MERCATOR_HALF_EQUATOR_M).abs() < 1e-3);
    }
}
