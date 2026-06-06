/// Open OLC free triangle scoring multiplier in points per kilometre.
pub(super) const FREE_TRIANGLE_OPEN_MULTIPLIER: f64 = 1.2;

/// Closed OLC free triangle scoring multiplier in points per kilometre.
pub(super) const FREE_TRIANGLE_CLOSED_MULTIPLIER: f64 = 1.4;

/// Free triangles below this fraction of the flight's free-distance result are
/// not worth returning from FD-aware scoring.
pub(super) const MIN_FREE_TO_FREE_DISTANCE_RATIO: f64 = 0.25;

/// Minimum point count for the cheap Free Triangle probe.
pub(super) const PROBE_MIN_POINTS: usize = 100;
/// Maximum point count for the cheap Free Triangle probe.
pub(super) const PROBE_MAX_POINTS: usize = 200;
/// Smallest RDP tolerance to try for the cheap Free Triangle probe.
pub(super) const PROBE_MIN_TOLERANCE_M: f64 = 0.0;
/// Biggest RDP tolerance to try for the cheap Free Triangle probe.
pub(super) const PROBE_MAX_TOLERANCE_M: f64 = 500.0;

/// Relaxed closure threshold for simplified-track Free Triangle probes.
pub(super) const FREE_TRIANGLE_CLOSURE_PREFILTER: f64 = 0.25;

/// Coarse Free Triangle results below this fraction of free distance are
/// skipped before the full search.
pub(super) const MIN_COARSE_TO_FREE_DISTANCE_RATIO: f64 = 0.50;
