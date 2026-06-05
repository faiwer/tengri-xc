use super::consts::E5_TO_DEGREES;

/// FCC-formula surface distance in kilometres between two E5 lat/lon points.
///
/// Uses the five-term Chebyshev approximation from the FCC Rules §73.208.
/// More accurate than a single-cosine approximation at the cost of a few
/// extra multiplications; still cheaper than Haversine (no asin).
pub fn fcc_distance_km(lat_a_e5: i32, lon_a_e5: i32, lat_b_e5: i32, lon_b_e5: i32) -> f64 {
    let delta_lat = (lat_b_e5 - lat_a_e5) as f64 * E5_TO_DEGREES;
    let delta_lon = (lon_b_e5 - lon_a_e5) as f64 * E5_TO_DEGREES;
    let mid_lat_rad = ((lat_a_e5 + lat_b_e5) as f64 * 0.5 * E5_TO_DEGREES).to_radians();
    let cos_mid_lat = mid_lat_rad.cos();
    let cos_2_mid_lat = 2.0 * cos_mid_lat * cos_mid_lat - 1.0;
    let cos_3_mid_lat = cos_mid_lat * (2.0 * cos_2_mid_lat - 1.0);
    let cos_4_mid_lat = 2.0 * cos_2_mid_lat * cos_2_mid_lat - 1.0;
    let cos_5_mid_lat = 2.0 * cos_2_mid_lat * cos_3_mid_lat - cos_mid_lat;
    let lat_km_per_deg = 111.13209 - 0.566605 * cos_2_mid_lat + 0.00120 * cos_4_mid_lat;
    let lon_km_per_deg =
        111.41513 * cos_mid_lat - 0.09455 * cos_3_mid_lat + 0.00012 * cos_5_mid_lat;
    let lat_km = lat_km_per_deg * delta_lat;
    let lon_km = lon_km_per_deg * delta_lon;

    (lat_km * lat_km + lon_km * lon_km).sqrt()
}
