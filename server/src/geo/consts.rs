/// Mean Earth radius in metres. WGS-84 reference defines a slightly
/// different value at the equator (6 378 137 m) and the poles
/// (6 356 752 m); we use the IUGG mean radius, which keeps spherical-
/// model error below ~0.3 % anywhere on the globe.
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Conversion factor from E5 micro-degrees to radians, in one multiply.
/// Equivalent to `(value as f64) / 1e5 * π / 180.0`.
pub const E5_TO_RAD: f64 = std::f64::consts::PI / 180.0 / 1e5;
