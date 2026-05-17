use super::consts::{E5_TO_RAD, EARTH_RADIUS_M};

/// Fast approximate surface distance in metres between two E5 lat/lon points.
/// Relatively accurate for short distances at any latitude. Even Alaska.
/// Since it has only 1 cos it's ~6x faster than the Haversine formula.
pub fn approximate_distance_m(lat_a_e5: i32, lon_a_e5: i32, lat_b_e5: i32, lon_b_e5: i32) -> f64 {
    let mid_lat = (lat_a_e5 + lat_b_e5) as f64 * 0.5 * E5_TO_RAD;
    let dlat = (lat_b_e5 - lat_a_e5) as f64 * E5_TO_RAD;
    let dlon = (lon_b_e5 - lon_a_e5) as f64 * E5_TO_RAD * mid_lat.cos();

    (dlat * dlat + dlon * dlon).sqrt() * EARTH_RADIUS_M
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e5(deg: f64) -> i32 {
        (deg * 1e5).round() as i32
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
}
