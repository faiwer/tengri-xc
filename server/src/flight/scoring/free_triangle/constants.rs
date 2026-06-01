/// Open OLC free triangle scoring multiplier in points per kilometre.
pub(super) const FREE_TRIANGLE_OPEN_MULTIPLIER: f64 = 1.2;

/// Closed OLC free triangle scoring multiplier in points per kilometre.
pub(super) const FREE_TRIANGLE_CLOSED_MULTIPLIER: f64 = 1.4;

/// Free triangles below this fraction of the flight's free-distance result are
/// not worth returning from FD-aware scoring.
pub(super) const MIN_FREE_TO_FREE_DISTANCE_RATIO: f64 = 0.25;
