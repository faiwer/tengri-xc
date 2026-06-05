//! Takeoff and landing detection for parsed flight tracks.
//!
//! The public detector returns the index window that best represents the
//! airborne portion of a track. It first uses an altitude-aware pass when GPS
//! or pressure altitude has enough range to distinguish flight from ground
//! movement:
//!
//! 1. Pick pressure altitude when it is usable, otherwise GPS altitude.
//! 2. Build moving averages for horizontal speed and absolute vertical speed.
//! 3. Detect sustained flight from simultaneous horizontal and vertical motion.
//! 4. Detect landing from sustained low horizontal and vertical motion.
//! 5. When several airborne candidates exist, keep the one with the greatest
//!    path distance.
//!
//! Tracks without useful altitude data fall back to the horizontal-speed HMM
//! path: per-fix speed emissions, Viterbi smoothing, short-stop re-merge, then
//! boundary scan for takeoff and landing indices.

use crate::types::{Track, TrackPoint};
use tengri_geo::{approximate_distance_m, haversine_m};

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
pub fn find_flight_window(track: &Track) -> Option<FlightWindow> {
    let n = track.points.len();
    if n < 2 {
        return None;
    }

    if let Some(window) = find_altitude_flight_window(&track.points) {
        return Some(window);
    }

    find_horizontal_flight_window(&track.points)
}

fn find_horizontal_flight_window(points: &[TrackPoint]) -> Option<FlightWindow> {
    let emissions = build_emissions(points);
    let smoothed = viterbi_decode(&emissions, &HMM);
    let flying = apply_min_landing_time(&smoothed, points);
    scan_boundaries(&flying)
}

/// Moving-average period for altitude-aware speeds; this smooths the samples,
/// it is not the duration required to split a flight window.
const ALTITUDE_MA_PERIOD_S: u32 = 10;
/// Horizontal speed required to start an airborne candidate.
const FLIGHT_INITIAL_HSPEED_MPS: f64 = 5.0;
/// Horizontal speed that keeps an airborne candidate alive; vertical speed
/// above `FLIGHT_CONTINUE_VSPEED_MPS` keeps it alive too.
const FLIGHT_CONTINUE_HSPEED_MPS: f64 = 1.5;
/// Absolute vertical speed required to start an airborne candidate.
const FLIGHT_INITIAL_VSPEED_MPS: f64 = 0.9;
/// Absolute vertical speed that keeps an airborne candidate alive; horizontal
/// speed above `FLIGHT_CONTINUE_HSPEED_MPS` keeps it alive too.
const FLIGHT_CONTINUE_VSPEED_MPS: f64 = 0.05;
/// Continuous flight-like movement required before accepting takeoff.
const FLIGHT_DETECT_TIME_S: u32 = 60;
/// Horizontal speed below this threshold can count as landed.
const GROUND_MAX_HSPEED_MPS: f64 = 2.5;
/// Absolute vertical speed below this threshold can count as landed.
const GROUND_MAX_VSPEED_MPS: f64 = 0.1;
/// Continuous ground-like movement required before accepting landing.
const GROUND_DETECT_TIME_S: u32 = 20;
/// Minimum altitude range before vertical movement is trusted: 100 dm = 10 m.
const MIN_USABLE_ALTITUDE_RANGE_DM: i32 = 100;
/// Minimum non-null altitude samples needed before vertical movement is trusted.
const MIN_USABLE_ALTITUDE_VALUES: usize = 4;

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

/// Return the best altitude-aware flight window, if altitude data is usable.
///
/// The detector builds flight and ground masks from smoothed horizontal and
/// vertical speeds, converts those masks into candidate windows, then keeps the
/// candidate with the greatest path distance.
fn find_altitude_flight_window(points: &[TrackPoint]) -> Option<FlightWindow> {
    let altitude = select_usable_altitude_m(points)?;
    let (hma, vma) = compute_smoothed_speed_series(points, &altitude);
    let state_flight = detect_sustained_flight(points, &hma, &vma);
    let state_ground = detect_sustained_ground(points, &hma, &vma);
    let windows = detect_launch_landing(points, &state_flight, &state_ground);

    windows.into_iter().max_by(|a, b| {
        compute_window_path_distance_m(points, *a)
            .total_cmp(&compute_window_path_distance_m(points, *b))
    })
}

/// Select the altitude series used by the altitude-aware detector.
///
/// Pressure altitude wins when every fix has it and it is varied enough to be
/// useful. Otherwise GPS altitude is used if it passes the same usability check.
/// Returned values are metres, converted from the track's decimetre storage.
fn select_usable_altitude_m(points: &[TrackPoint]) -> Option<Vec<f64>> {
    let pressure: Option<Vec<i32>> = points.iter().map(|point| point.pressure_alt).collect();
    if let Some(pressure) = pressure
        && is_altitude_usable(&pressure)
    {
        return Some(pressure.into_iter().map(|alt| alt as f64 / 10.0).collect());
    }

    let geo: Vec<i32> = points.iter().map(|point| point.geo_alt).collect();
    is_altitude_usable(&geo).then(|| geo.into_iter().map(|alt| alt as f64 / 10.0).collect())
}

/// Return whether an altitude series has enough signal for vertical speed checks.
///
/// A usable series needs enough samples, at least 10 m of total range, and
/// enough distinct values to avoid treating flat placeholder altitude as real
/// vertical movement.
fn is_altitude_usable(altitude_dm: &[i32]) -> bool {
    if altitude_dm.len() < 5 {
        return false;
    }

    let min = altitude_dm.iter().copied().min().unwrap_or(0);
    let max = altitude_dm.iter().copied().max().unwrap_or(0);
    if max.saturating_sub(min) < MIN_USABLE_ALTITUDE_RANGE_DM {
        return false;
    }

    let mut distinct = altitude_dm.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    distinct.len() >= MIN_USABLE_ALTITUDE_VALUES
}

/// Return `(hma_mps, vma_mps)` aligned to fixes: smoothed horizontal speed and
/// smoothed absolute vertical speed, both in metres per second.
fn compute_smoothed_speed_series(
    points: &[TrackPoint],
    altitude_m: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let n = points.len();
    let mut hspeed = vec![0.0; n];
    let mut vspeed = vec![0.0; n];

    for i in 1..n {
        let dt = points[i].time.saturating_sub(points[i - 1].time);
        if dt == 0 {
            hspeed[i] = hspeed[i - 1];
            vspeed[i] = vspeed[i - 1];
            continue;
        }

        hspeed[i] = approximate_distance_m(
            points[i - 1].lat,
            points[i - 1].lon,
            points[i].lat,
            points[i].lon,
        ) / f64::from(dt);
        vspeed[i] = (altitude_m[i] - altitude_m[i - 1]) / f64::from(dt);
    }

    let mut hma = vec![0.0; n];
    let mut vma = vec![0.0; n];
    let half_window_s = ALTITUDE_MA_PERIOD_S / 2;

    for i in 0..n {
        let now = points[i].time;
        let mut start = i;
        while start > 0 && points[start].time > now.saturating_sub(half_window_s) {
            start -= 1;
        }
        let mut end = i;
        while end < n - 1 && points[end].time < now.saturating_add(half_window_s) {
            end += 1;
        }

        let count = (end - start + 1) as f64;
        for j in start..=end {
            hma[i] += hspeed[j];
            vma[i] += vspeed[j].abs();
        }
        hma[i] /= count;
        vma[i] /= count;
    }

    (hma, vma)
}

/// Return a flight mask from smoothed speeds.
///
/// A run starts when both horizontal and absolute vertical speed cross the
/// initial flight thresholds. It stays alive while both speeds remain above the
/// lower continuation thresholds; once the run lasts long enough, the whole run
/// is marked as flight-like.
fn detect_sustained_flight(points: &[TrackPoint], hma: &[f64], vma: &[f64]) -> Vec<bool> {
    let n = points.len();
    let mut out = vec![false; n];
    let mut start: Option<usize> = None;

    for i in 0..n.saturating_sub(1) {
        if start.is_none()
            && hma[i] > FLIGHT_INITIAL_HSPEED_MPS
            && vma[i] > FLIGHT_INITIAL_VSPEED_MPS
        {
            start = Some(i);
        }

        if let Some(start_idx) = start {
            if hma[i] > FLIGHT_CONTINUE_HSPEED_MPS && vma[i] > FLIGHT_CONTINUE_VSPEED_MPS {
                if points[i].time > points[start_idx].time.saturating_add(FLIGHT_DETECT_TIME_S) {
                    out[start_idx..=i].fill(true);
                }
            } else {
                start = None;
            }
        }
    }

    out
}

/// Return a ground mask from smoothed speeds.
///
/// A run starts when both horizontal and absolute vertical speed are below the
/// ground thresholds. Once that low-motion run lasts long enough, the whole run
/// is marked as ground-like.
fn detect_sustained_ground(points: &[TrackPoint], hma: &[f64], vma: &[f64]) -> Vec<bool> {
    let n = points.len();
    let mut out = vec![false; n];
    let mut start: Option<usize> = None;

    for i in 0..n.saturating_sub(1) {
        if start.is_none() && hma[i] < GROUND_MAX_HSPEED_MPS && vma[i] < GROUND_MAX_VSPEED_MPS {
            start = Some(i);
        }

        if let Some(start_idx) = start {
            if hma[i] < GROUND_MAX_HSPEED_MPS && vma[i] < GROUND_MAX_VSPEED_MPS {
                if points[i].time > points[start_idx].time.saturating_add(GROUND_DETECT_TIME_S) {
                    out[start_idx..=i].fill(true);
                }
            } else {
                start = None;
            }
        }
    }

    out
}

/// Convert flight/ground masks into candidate takeoff and landing windows.
///
/// The scan finds each flight-like run, extends its start backward to the
/// previous ground-like fix, extends its end forward to the next ground-like
/// fix, then resumes after that landing so the same run is not reported twice.
fn detect_launch_landing(
    points: &[TrackPoint],
    state_flight: &[bool],
    state_ground: &[bool],
) -> Vec<FlightWindow> {
    let n = points.len();
    let mut windows = Vec::new();
    let scan_end = n.saturating_sub(1);
    let landing_search_end = n.saturating_sub(2);
    let mut i = 0;

    while i < scan_end {
        if !state_flight[i] {
            i += 1;
            continue;
        }

        let mut launch = i;
        while launch > 0 && !state_ground[launch] {
            launch -= 1;
        }

        let mut landing = i;
        while landing < landing_search_end && !state_ground[landing] {
            landing += 1;
        }

        windows.push(FlightWindow {
            takeoff_idx: launch,
            landing_idx: landing,
        });
        i = landing.saturating_add(1);
    }

    windows
}

/// Rank candidate windows by flown path distance inside their boundaries.
fn compute_window_path_distance_m(points: &[TrackPoint], window: FlightWindow) -> f64 {
    if window.landing_idx <= window.takeoff_idx {
        return 0.0;
    }

    (window.takeoff_idx + 1..=window.landing_idx)
        .map(|i| {
            approximate_distance_m(
                points[i - 1].lat,
                points[i - 1].lon,
                points[i].lat,
                points[i].lon,
            )
        })
        .sum()
}

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

/// Pass 2: only keep an interior low-speed run as flying when it is shorter
/// than `MIN_LANDING_TIME_S`, measured by `point.time` deltas. Leading ground
/// time is not part of the flight.
fn apply_min_landing_time(smoothed: &[u8], points: &[TrackPoint]) -> Vec<bool> {
    let n = smoothed.len();
    let mut flying: Vec<bool> = smoothed.iter().map(|&state| state == 1).collect();

    let mut i = 0;
    while i < n {
        if smoothed[i] != 0 {
            i += 1;
            continue;
        }

        let start = i;
        while i < n && smoothed[i] == 0 {
            i += 1;
        }
        let end = i;

        if start == 0 || end == n {
            continue;
        }

        let dt = points[end].time.saturating_sub(points[start].time);
        if dt < MIN_LANDING_TIME_S {
            flying[start..end].fill(true);
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
                tas: None,
            })
            .collect();
        let start_time = points.first().map(|p| p.time).unwrap_or(0);
        Track { start_time, points }
    }

    fn track_with_geo_alt(samples: &[(u32, f64, f64, i32)]) -> Track {
        let points: Vec<TrackPoint> = samples
            .iter()
            .map(|&(t, lat, lon, geo_alt_m)| TrackPoint {
                time: t,
                lat: (lat * 1e5).round() as i32,
                lon: (lon * 1e5).round() as i32,
                geo_alt: geo_alt_m * 10,
                pressure_alt: None,
                tas: None,
            })
            .collect();
        let start_time = points.first().map(|p| p.time).unwrap_or(0);
        Track { start_time, points }
    }

    fn track_with_pressure_alt(samples: &[(u32, f64, f64, i32)]) -> Track {
        let points: Vec<TrackPoint> = samples
            .iter()
            .map(|&(t, lat, lon, pressure_alt_m)| TrackPoint {
                time: t,
                lat: (lat * 1e5).round() as i32,
                lon: (lon * 1e5).round() as i32,
                geo_alt: 0,
                pressure_alt: Some(pressure_alt_m * 10),
                tas: None,
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

    fn samples_with_alt(
        samples: &[(u32, f64, f64)],
        alt: impl Fn(u32, usize) -> i32,
    ) -> Vec<(u32, f64, f64, i32)> {
        samples
            .iter()
            .enumerate()
            .map(|(idx, &(t, lat, lon))| (t, lat, lon, alt(t, idx)))
            .collect()
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

    #[test]
    fn short_lead_in_is_excluded_from_flight() {
        let mut samples = stationary(0, 60, 47.0, 8.0);
        samples.extend(leg_eastbound(60, 600, 47.0, 8.0, 30.0));
        let last_lon = samples.last().unwrap().2;
        samples.extend(stationary(660, 600, 47.0, last_lon));
        let t = track(&samples);
        let w = find_flight_window(&t).expect("should detect flight");
        assert!(
            (55..=65).contains(&w.takeoff_idx),
            "takeoff near the stand→fly edge, got {}",
            w.takeoff_idx
        );
        assert!(
            (655..=665).contains(&w.landing_idx),
            "landing near the fly→stand edge, got {}",
            w.landing_idx
        );
    }

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

    #[test]
    fn altitude_detector_uses_gps_altitude_without_pressure_altitude() {
        let mut samples = stationary(0, 40, 47.0, 8.0);
        samples.extend(leg_eastbound(40, 180, 47.0, 8.0, 45.0));
        let alt_samples = samples_with_alt(&samples, |t, _| {
            if t < 40 {
                1_000
            } else {
                1_000 + (t as i32 - 40) * 2
            }
        });
        let t = track_with_geo_alt(&alt_samples);

        let w = find_flight_window(&t).expect("should detect flight");

        assert!(
            (35..=45).contains(&w.takeoff_idx),
            "takeoff near the altitude-active edge, got {}",
            w.takeoff_idx
        );
    }

    #[test]
    fn altitude_detector_prefers_pressure_altitude_when_present() {
        let mut samples = stationary(0, 40, 47.0, 8.0);
        samples.extend(leg_eastbound(40, 180, 47.0, 8.0, 45.0));
        let alt_samples = samples_with_alt(&samples, |t, _| {
            if t < 40 {
                1_000
            } else {
                1_000 + (t as i32 - 40) * 2
            }
        });
        let t = track_with_pressure_alt(&alt_samples);

        let w = find_flight_window(&t).expect("should detect flight");

        assert!(
            (35..=45).contains(&w.takeoff_idx),
            "takeoff near the altitude-active edge, got {}",
            w.takeoff_idx
        );
    }

    #[test]
    fn altitude_detector_selects_stronger_candidate_window() {
        let mut samples = stationary(0, 40, 47.0, 8.0);
        samples.extend(leg_eastbound(40, 120, 47.0, 8.0, 30.0));
        let first_end_lon = samples.last().unwrap().2;
        samples.extend(stationary(160, 60, 47.0, first_end_lon));
        samples.extend(leg_eastbound(220, 180, 47.0, first_end_lon, 45.0));
        let alt_samples = samples_with_alt(&samples, |t, _| match t {
            0..=39 => 1_000,
            40..=159 => 1_000 + (t as i32 - 40) * 2,
            160..=219 => 1_240,
            _ => 1_240 + (t as i32 - 220) * 2,
        });
        let t = track_with_geo_alt(&alt_samples);

        let w = find_flight_window(&t).expect("should detect flight");

        assert!(
            (215..=225).contains(&w.takeoff_idx),
            "takeoff should come from the longer second window, got {}",
            w.takeoff_idx
        );
    }

    #[test]
    fn flat_altitude_uses_horizontal_fallback() {
        let mut samples = stationary(0, 60, 47.0, 8.0);
        samples.extend(leg_eastbound(60, 600, 47.0, 8.0, 30.0));
        let alt_samples = samples_with_alt(&samples, |_, _| 1_000);
        let t = track_with_geo_alt(&alt_samples);

        let w = find_flight_window(&t).expect("should detect flight");

        assert!(
            (55..=65).contains(&w.takeoff_idx),
            "flat altitude should fall back to horizontal detection, got {}",
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
