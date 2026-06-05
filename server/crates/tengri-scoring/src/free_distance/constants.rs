/// Since free distance is the simplest kind of route it has no boost multiplier.
pub(super) const FREE_DISTANCE_MULTIPLIER: f64 = 1.0;

/// Preferred number of points in the RDP-simplified track before we run DP (any
/// iteration). The number is chosen empirically to be a good trade-off between
/// accuracy and speed.
pub(super) const RDP_TARGET_POINTS: usize = 500;

/// Spread around RDP_TARGET_POINTS to relax binary search. `0.1` means ±10%.
pub(super) const RDP_TARGET_SPREAD: f64 = 0.1;

/// Smallest RDP tolerance we try, in meters. Lower tolerance keeps more track
/// detail. But it makes no sense to drop it to 0, because nobody cares about
/// spiral sizes when scoring the track free distance. The lower this value, the
/// the longer the RDP track becomes. That means the longer the DP run becomes.
pub(super) const RDP_MIN_TOLERANCE_M: f64 = 25.0;

/// Biggest RDP tolerance we try, in meters. Too big value may significantly
/// change the track shape. Found empirically.
pub(super) const RDP_MAX_TOLERANCE_M: f64 = 500.0;

/// Determines how many points to keep around each found solution point in % of
/// the total track points. Found empirically.
pub(super) const REFINE_START_WINDOW_PERCENT: f64 = 1.5;

/// Stop simplifying the track once it is this small. At that size we can run DP
/// directly on the raw indexes and receive the most accurate result. No need to
/// iterate further. It's practially instant.
pub(super) const REFINE_MIN_WINDOW_POINTS: usize = 150;
