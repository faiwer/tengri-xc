/// Minimum length of each FAI triangle side as a fraction of total triangle
/// distance.
pub(super) const MIN_SIDE: f64 = 0.28;

/// Default per-side floor for scoring FAI triangles, in kilometres. Used to
/// avoid computing too tiny triangles where a single spiral loop would be be
/// considered a valid FAI triangle.
pub(super) const DEFAULT_MIN_SCORING_SIDE_KM: f64 = 1.4;

/// Open OLC FAI triangle scoring multiplier in points per kilometre.
pub(super) const FAI_TRIANGLE_OPEN_MULTIPLIER: f64 = 1.4;

/// Closed OLC FAI triangle scoring multiplier in points per kilometre.
pub(super) const FAI_TRIANGLE_CLOSED_MULTIPLIER: f64 = 1.6;

/// Before we start computing FAI triangles, we search for a simpler FAI
/// triangle that is at least this fraction of the flight's free-distance result.
/// If we find no such triangle, we skip the real FAI triangle search.
pub(super) const MIN_FAI_TO_FREE_DISTANCE_RATIO: f64 = 0.25;

/// The closure threshold for open OLC FAI triangles. The B-C leg must be at
/// least this fraction of the flight's free-distance result.
pub const FAI_CLOSURE_OPEN: f64 = 0.2;

/// Maximum closure gap for closed OLC FAI triangles, as a fraction of triangle
/// distance.
pub const FAI_CLOSURE_CLOSED: f64 = 0.05;

/// Relaxed closure threshold for simplified-track prefilter probes.
///
/// The prefilter runs on an RDP-simplified vertex set; simplification can shift
/// a triangle's closure by a couple of metres and tip a barely-valid candidate
/// over the strict 20% line. Probing at 25% rescues those true positives. The
/// side effect is that the prefilter can also return a triangle whose closure
/// is in (20%, 25%] -- geometrically real, but one the strict solver would
/// reject. So `coarse_fai_triangle_m` is a feasibility signal, not a lower
/// bound on the exact result.
pub const FAI_CLOSURE_PREFILTER: f64 = 0.25;
