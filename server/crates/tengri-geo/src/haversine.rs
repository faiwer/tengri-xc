//! Great-circle distance between two lat/lon points on a spherical Earth.
//!
//! The Haversine formulation is preferred over the spherical law of
//! cosines because `asin(sqrt(...))` stays accurate down to single-metre
//! distances, where `acos` of values near 1 collapses precision. That
//! matters here: consecutive 1 Hz GPS fixes are typically tens of metres
//! apart and we want their distances to round-trip cleanly.
//!
//! Inputs are E5 micro-degrees — the wire/storage unit used throughout
//! the server. Keeping the integer signature avoids `f64` conversions at
//! every call site; converting once inside the function is the same cost
//! and stops the unit from leaking out.
//!
//! Result is a 2D surface distance on a sphere of radius
//! [`EARTH_RADIUS_M`]. No altitude term — that's a different operation.
//! Spherical (not WGS-84) Earth: worst-case error ~0.3 % between equator
//! and poles, which is irrelevant for ground-speed thresholding and
//! per-leg track distance.

use super::consts::{E5_TO_RAD, EARTH_RADIUS_M};

/// Great-circle distance in metres between two points given as
/// E5 micro-degrees (`degrees × 10⁵`, the project-wide wire unit).
pub fn haversine_m(lat_a_e5: i32, lon_a_e5: i32, lat_b_e5: i32, lon_b_e5: i32) -> f64 {
    let lat_a = lat_a_e5 as f64 * E5_TO_RAD;
    let lat_b = lat_b_e5 as f64 * E5_TO_RAD;
    let dlat = (lat_b_e5 - lat_a_e5) as f64 * E5_TO_RAD;
    let dlon = (lon_b_e5 - lon_a_e5) as f64 * E5_TO_RAD;
    let s_lat = (dlat * 0.5).sin();
    let s_lon = (dlon * 0.5).sin();
    let a = s_lat * s_lat + lat_a.cos() * lat_b.cos() * s_lon * s_lon;
    let c = 2.0 * a.sqrt().asin();
    EARTH_RADIUS_M * c
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convert decimal degrees to E5 micro-degrees for test fixtures.
    fn e5(deg: f64) -> i32 {
        (deg * 1e5).round() as i32
    }

    /// Two coincident points are exactly zero metres apart.
    #[test]
    fn identical_points_are_zero() {
        let d = haversine_m(e5(47.3769), e5(8.5417), e5(47.3769), e5(8.5417));
        assert_eq!(d, 0.0);
    }

    /// Geneva (46.2044°, 6.1432°) ↔ Zurich (47.3769°, 8.5417°).
    /// Reference distance ≈ 224 km. We accept ±1 km for the spherical
    /// approximation vs the canonical WGS-84 figure.
    #[test]
    fn geneva_to_zurich_known_distance() {
        let d = haversine_m(e5(46.2044), e5(6.1432), e5(47.3769), e5(8.5417));
        let km = d / 1000.0;
        assert!(
            (223.0..=225.0).contains(&km),
            "Geneva→Zurich expected ~224 km, got {km:.3} km"
        );
    }

    /// Symmetric: d(A, B) == d(B, A) to floating-point exactness.
    #[test]
    fn symmetric() {
        let d_ab = haversine_m(e5(47.0), e5(8.0), e5(47.5), e5(9.0));
        let d_ba = haversine_m(e5(47.5), e5(9.0), e5(47.0), e5(8.0));
        assert_eq!(d_ab, d_ba);
    }

    /// One degree of latitude on the meridian is ~111.195 km on a sphere
    /// of radius 6 371 km (πR/180). Anchors the formula and the radius
    /// constant.
    #[test]
    fn one_degree_of_latitude_is_about_111km() {
        let d = haversine_m(e5(0.0), e5(0.0), e5(1.0), e5(0.0));
        let km = d / 1000.0;
        assert!(
            (111.0..=111.4).contains(&km),
            "1° lat expected ~111.2 km, got {km:.3} km"
        );
    }

    /// Same Δlon collapses to ~zero distance at the poles and is widest
    /// at the equator. Verifies the cos(lat)·cos(lat) longitude weighting.
    #[test]
    fn longitude_distance_shrinks_with_latitude() {
        let at_equator = haversine_m(e5(0.0), e5(0.0), e5(0.0), e5(1.0));
        let at_47 = haversine_m(e5(47.0), e5(0.0), e5(47.0), e5(1.0));
        let near_pole = haversine_m(e5(89.0), e5(0.0), e5(89.0), e5(1.0));
        assert!(at_equator > at_47);
        assert!(at_47 > near_pole);
        assert!(near_pole < 2_000.0, "1° lon at 89° should be < 2 km");
    }
}
