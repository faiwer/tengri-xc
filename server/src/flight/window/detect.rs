//! Takeoff/landing detection on a parsed [`Track`].
//!
//! Port of `igc_lib._compute_flight` + `_compute_takeoff_landing`. The
//! algorithm is well-documented in the Python source; this is a faithful
//! Rust translation:
//!
//! 1. Per-fix ground-speed emissions (`1` if gsp > 15 km/h, else `0`).
//! 2. Viterbi smoothing with sticky transitions (handles GPS noise + lone
//!    outliers).
//! 3. `min_landing_time` re-merge: a `0` segment shorter than 300 s is
//!    folded back into the surrounding `1` (handles low-speed scratching,
//!    near-ground thermalling, brief touch-and-relaunch).
//! 4. Boundary scan: `takeoff_idx` is the first flying fix; `landing_idx`
//!    is the first non-flying fix after the last flying one (or the very
//!    last fix if the log ended mid-flight).
//!
//! Constants are hardcoded (matching `igc_lib` defaults) and only become a
//! parameter struct when there's a concrete reason to vary them.

use crate::flight::types::{Track, TrackPoint};
use crate::geo::haversine_m;

use super::viterbi::{HmmParams, decode as viterbi_decode};

/// Inclusive index pair pointing into [`Track::points`].
///
/// `landing_idx` follows `igc_lib` semantics: it is the first *non-flying*
/// fix after the last flying one, so it sits one past the in-air segment.
/// If the log ended while the pilot was still airborne, `landing_idx`
/// equals the final point's index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlightWindow {
    pub takeoff_idx: usize,
    pub landing_idx: usize,
}

/// Detect the takeoff/landing window for `track`. Returns `None` when no
/// flying segment is found (track too short, all stationary, etc.).
///
/// Reference-algorithm quirk worth knowing about: a non-flying run shorter
/// than `MIN_LANDING_TIME_S` gets reabsorbed into the surrounding flight,
/// including a *leading* run. So a track that opens with 60 s of standing
/// fixes immediately followed by takeoff yields `takeoff_idx == 0`, not
/// the index where movement actually starts. This matches `igc_lib` and is
/// usually what callers want (the few seconds of fumbling at launch are
/// part of "the flight" for most purposes).
pub fn find_flight_window(track: &Track) -> Option<FlightWindow> {
    let n = track.points.len();
    if n < 2 {
        return None;
    }

    let emissions = build_emissions(&track.points);
    let smoothed = viterbi_decode(&emissions, &HMM);
    let flying = apply_min_landing_time(&smoothed, &track.points);
    scan_boundaries(&flying)
}

/// Minimum ground speed (km/h) to treat a fix as "flying". Matches
/// `igc_lib.FlightParsingConfig.min_gsp_flight`.
const MIN_GSP_FLIGHT_KMH: f64 = 15.0;

/// Minimum continuous low-speed duration (seconds) to count as a real
/// landing. Shorter gaps are reabsorbed into the in-air segment. Matches
/// `igc_lib.FlightParsingConfig.min_landing_time` (5 minutes).
const MIN_LANDING_TIME_S: u32 = 300;

/// HMM parameters lifted verbatim from `igc_lib._compute_flight`.
const HMM: HmmParams = HmmParams {
    init: [0.80, 0.20],
    transition: [[0.9995, 0.0005], [0.0005, 0.9995]],
    emission: [[0.8, 0.2], [0.2, 0.8]],
};

/// Per-fix emissions. The pair-based ground speed for index `i` (i ≥ 1)
/// uses the segment from `i-1` to `i`. The emission at index 0 reuses the
/// emission at index 1 (the natural mirror of `igc_lib`, which derives gsp
/// the same way and aligns it forward).
fn build_emissions(points: &[TrackPoint]) -> Vec<u8> {
    let n = points.len();
    let mut out = vec![0u8; n];
    for i in 1..n {
        let prev = &points[i - 1];
        let cur = &points[i];
        let dt = cur.time.saturating_sub(prev.time);
        if dt == 0 {
            out[i] = 0;
            continue;
        }
        let dist_m = haversine_m(prev.lat, prev.lon, cur.lat, cur.lon);
        let kmh = dist_m / dt as f64 * 3.6;
        out[i] = if kmh > MIN_GSP_FLIGHT_KMH { 1 } else { 0 };
    }
    out[0] = out[1];
    out
}

/// Pass 2: only emit landings (`0`) if the contiguous low-speed run lasts
/// at least `MIN_LANDING_TIME_S`, measured by `point.time` deltas. Mirrors
/// the lookahead-then-decide loop in `igc_lib._compute_flight`.
fn apply_min_landing_time(smoothed: &[u8], points: &[TrackPoint]) -> Vec<bool> {
    let n = smoothed.len();
    let mut flying = vec![false; n];
    let mut ignore_next_downtime = false;
    let mut apply_next_downtime = false;

    for i in 0..n {
        if smoothed[i] == 1 {
            flying[i] = true;
            ignore_next_downtime = false;
            apply_next_downtime = false;
            continue;
        }

        if apply_next_downtime {
            flying[i] = false;
            continue;
        }
        if ignore_next_downtime {
            flying[i] = true;
            continue;
        }

        let mut j = i + 1;
        while j < n && smoothed[j] != 1 {
            j += 1;
        }
        if j == n {
            apply_next_downtime = true;
            flying[i] = false;
            continue;
        }

        let dt = points[j].time.saturating_sub(points[i].time);
        if dt >= MIN_LANDING_TIME_S {
            apply_next_downtime = true;
            flying[i] = false;
        } else {
            ignore_next_downtime = true;
            flying[i] = true;
        }
    }
    flying
}

/// `igc_lib._compute_takeoff_landing` equivalent: walk the boolean array
/// once and pick the first flying-edge / first post-flying-edge.
fn scan_boundaries(flying: &[bool]) -> Option<FlightWindow> {
    let mut takeoff_idx: Option<usize> = None;
    let mut landing_idx: Option<usize> = None;
    let mut was_flying = false;
    for (i, &f) in flying.iter().enumerate() {
        if f && takeoff_idx.is_none() {
            takeoff_idx = Some(i);
        }
        if !f && was_flying {
            landing_idx = Some(i);
        }
        was_flying = f;
    }

    let takeoff_idx = takeoff_idx?;
    let landing_idx = landing_idx.unwrap_or(flying.len() - 1);
    Some(FlightWindow {
        takeoff_idx,
        landing_idx,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic Track from `(t, lat_deg, lon_deg)` triples.
    /// Altitudes are zeroed; they don't enter the detection algorithm.
    fn track(samples: &[(u32, f64, f64)]) -> Track {
        let points: Vec<TrackPoint> = samples
            .iter()
            .map(|&(t, lat, lon)| TrackPoint {
                time: t,
                lat: (lat * 1e5).round() as i32,
                lon: (lon * 1e5).round() as i32,
                geo_alt: 0,
                pressure_alt: None,
            })
            .collect();
        let start_time = points.first().map(|p| p.time).unwrap_or(0);
        Track { start_time, points }
    }

    /// Linear constant-speed leg of `n` 1 Hz samples, eastbound from
    /// `(start_lat, start_lon)`. `kmh` controls the per-step longitude
    /// delta (using a flat-earth approximation: at 47° N, 1° lon ≈
    /// 75.9 km, but we just pick a delta that gives the requested speed
    /// at the given latitude).
    fn leg_eastbound(
        t0: u32,
        n: usize,
        start_lat: f64,
        start_lon: f64,
        kmh: f64,
    ) -> Vec<(u32, f64, f64)> {
        let m_per_step = kmh * 1000.0 / 3600.0;
        let m_per_deg_lon = 111_320.0 * (start_lat * std::f64::consts::PI / 180.0).cos();
        let dlon = m_per_step / m_per_deg_lon;
        (0..n)
            .map(|i| (t0 + i as u32, start_lat, start_lon + i as f64 * dlon))
            .collect()
    }

    fn stationary(t0: u32, n: usize, lat: f64, lon: f64) -> Vec<(u32, f64, f64)> {
        (0..n).map(|i| (t0 + i as u32, lat, lon)).collect()
    }

    #[test]
    fn empty_track_returns_none() {
        let t = track(&[]);
        assert_eq!(find_flight_window(&t), None);
    }

    #[test]
    fn single_point_track_returns_none() {
        let t = track(&[(0, 47.0, 8.0)]);
        assert_eq!(find_flight_window(&t), None);
    }

    #[test]
    fn pure_stand_returns_none() {
        let samples = stationary(0, 600, 47.0, 8.0);
        let t = track(&samples);
        assert_eq!(find_flight_window(&t), None);
    }

    #[test]
    fn pure_flight_covers_almost_all_of_it() {
        let samples = leg_eastbound(0, 600, 47.0, 8.0, 30.0);
        let t = track(&samples);
        let w = find_flight_window(&t).expect("should detect flight");
        assert!(
            w.takeoff_idx < 5,
            "takeoff near start, got {}",
            w.takeoff_idx
        );
        assert_eq!(w.landing_idx, 599, "track ended mid-flight");
    }

    /// `igc_lib` semantics: a leading non-flying run shorter than
    /// `MIN_LANDING_TIME_S` is reabsorbed into the upcoming flight, so
    /// `takeoff_idx` ends up at fix 0 even though no real movement
    /// happened yet. This is intentional in the reference algorithm
    /// (the lookahead in `apply_min_landing_time` makes no exception
    /// for the head of the track) and we match it.
    #[test]
    fn short_lead_in_is_reabsorbed_into_flight() {
        let mut samples = stationary(0, 60, 47.0, 8.0);
        samples.extend(leg_eastbound(60, 600, 47.0, 8.0, 30.0));
        let last_lon = samples.last().unwrap().2;
        samples.extend(stationary(660, 600, 47.0, last_lon));
        let t = track(&samples);
        let w = find_flight_window(&t).expect("should detect flight");
        assert_eq!(w.takeoff_idx, 0, "60 s lead-in < 300 s is reabsorbed");
        assert!(
            (655..=665).contains(&w.landing_idx),
            "landing near the fly→stand edge, got {}",
            w.landing_idx
        );
    }

    /// The complement of the previous test: a lead-in of ≥ 300 s is NOT
    /// reabsorbed, so `takeoff_idx` snaps to the first actually flying
    /// fix.
    #[test]
    fn long_lead_in_is_excluded_from_flight() {
        let mut samples = stationary(0, 600, 47.0, 8.0);
        samples.extend(leg_eastbound(600, 600, 47.0, 8.0, 30.0));
        let t = track(&samples);
        let w = find_flight_window(&t).expect("should detect flight");
        assert!(
            (595..=605).contains(&w.takeoff_idx),
            "takeoff near the stand→fly edge, got {}",
            w.takeoff_idx
        );
    }

    /// A 60-second slow patch in the middle of a flight (think: thermal
    /// scratching close to the ground) must NOT split the window — the
    /// 300 s `min_landing_time` reabsorbs it.
    #[test]
    fn brief_slow_patch_does_not_split_flight() {
        let mut samples = leg_eastbound(0, 300, 47.0, 8.0, 30.0);
        let mid_lon = samples.last().unwrap().2;
        samples.extend(stationary(300, 60, 47.0, mid_lon));
        samples.extend(leg_eastbound(360, 300, 47.0, mid_lon, 30.0));
        let t = track(&samples);
        let w = find_flight_window(&t).expect("should detect flight");
        assert!(w.takeoff_idx < 10);
        assert_eq!(
            w.landing_idx,
            samples.len() - 1,
            "brief stop must not end the flight"
        );
    }

    /// A real landing (≥ 300 s of low gsp) ends the window at the first
    /// non-flying fix after the last flying one. Matches `igc_lib`
    /// default `which_flight_to_pick = "concat"` walk semantics.
    #[test]
    fn long_stop_ends_the_flight() {
        let mut samples = leg_eastbound(0, 300, 47.0, 8.0, 30.0);
        let mid_lon = samples.last().unwrap().2;
        samples.extend(stationary(300, 600, 47.0, mid_lon));
        samples.extend(leg_eastbound(900, 300, 47.0, mid_lon, 30.0));
        let t = track(&samples);
        let w = find_flight_window(&t).expect("should detect flight");
        assert!(w.takeoff_idx < 10);
        assert!(
            (295..=310).contains(&w.landing_idx),
            "landing near the fly→stand edge of the first leg, got {}",
            w.landing_idx
        );
    }

    /// A track that ends mid-air pins the landing index to the last fix.
    #[test]
    fn mid_air_log_end_pins_landing_to_last_fix() {
        let mut samples = stationary(0, 60, 47.0, 8.0);
        samples.extend(leg_eastbound(60, 600, 47.0, 8.0, 30.0));
        let n = samples.len();
        let t = track(&samples);
        let w = find_flight_window(&t).expect("should detect flight");
        assert_eq!(w.landing_idx, n - 1);
    }

    /// Δt = 0 between consecutive fixes (bad recorder) must not panic
    /// and must not single-handedly flip state.
    #[test]
    fn duplicate_timestamps_are_handled() {
        let mut samples = stationary(0, 60, 47.0, 8.0);
        let leg = leg_eastbound(60, 600, 47.0, 8.0, 30.0);
        samples.extend(leg.clone());
        // Inject a duplicate timestamp in the middle of the flying leg.
        samples.insert(360, samples[360]);
        let t = track(&samples);
        let w = find_flight_window(&t).expect("should detect flight");
        assert!(w.takeoff_idx < 70);
        assert_eq!(w.landing_idx, t.points.len() - 1);
    }
}
