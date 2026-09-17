//! Which colour each stretch of the polyline gets. Mirrors the client's
//! `toPaths.ts`: grey before takeoff and after landing, the vario ramp in
//! between, and flat blue when the track can't support a vario reading.

use std::ops::Range;

use tengri_formats::{FlightWindow, Track};
use tengri_geo::{
    MAX_VARIO_FIX_INTERVAL_SECONDS, average_fix_interval, build_vario_segments, classify_buckets,
    vario_mps,
};

use super::paint::Rgb;

/// A stretch of fixes drawn in one colour. Neighbouring runs overlap by one
/// fix so the stroke has no gap at a colour change.
pub(super) struct ColorRun {
    pub color: Rgb,
    pub range: Range<usize>,
}

/// The client's `COLOR_GROUND`, for the walk up and the pack up.
const GROUND: Rgb = (0x9c, 0xa3, 0xaf);
/// The client's `COLOR_MISSING_ALTITUDE`.
const MISSING_ALTITUDE: Rgb = (0x3b, 0x82, 0xf6);

/// Indexed by `bucket + 5`, verbatim from the client's `VARIO_COLOR_RAMP`:
/// violet sink through yellow neutral to red climb, no green, so it reads as a
/// magnitude rather than as a good/bad signal.
const RAMP: [Rgb; 11] = [
    (0x7c, 0x3a, 0xed),
    (0x63, 0x66, 0xf1),
    (0x3b, 0x82, 0xf6),
    (0x38, 0xbd, 0xf8),
    (0x7d, 0xd3, 0xfc),
    (0xfd, 0xe0, 0x47),
    (0xfa, 0xcc, 0x15),
    (0xf5, 0x9e, 0x0b),
    (0xea, 0x58, 0x0c),
    (0xdc, 0x26, 0x26),
    (0x99, 0x1b, 0x1b),
];

pub(super) fn runs(track: &Track, window: Option<FlightWindow>) -> Vec<ColorRun> {
    let Some(last) = track.points.len().checked_sub(1) else {
        return Vec::new();
    };
    // Without a window the whole file is the flight, same fallback as the
    // takeoff/landing markers.
    let (takeoff, landing) = match window {
        Some(window) => {
            let takeoff = window.takeoff_idx.min(last);
            (takeoff, window.landing_idx.clamp(takeoff, last))
        }
        None => (0, last),
    };

    let mut runs = Vec::new();
    if takeoff > 0 {
        runs.push(ColorRun {
            color: GROUND,
            range: 0..takeoff + 1,
        });
    }
    runs.extend(flight_runs(track, takeoff, landing));
    if landing < last {
        runs.push(ColorRun {
            color: GROUND,
            range: landing..last + 1,
        });
    }
    runs
}

fn flight_runs(track: &Track, takeoff: usize, landing: usize) -> Vec<ColorRun> {
    if landing <= takeoff {
        return Vec::new();
    }

    let times: Vec<u32> = track.points.iter().map(|point| point.time).collect();
    let altitude = altitude_dm(track);
    let flight = takeoff..landing + 1;

    if !has_vario_data(&times, &altitude, flight.clone()) {
        return vec![ColorRun {
            color: MISSING_ALTITUDE,
            range: flight,
        }];
    }

    let buckets = classify_buckets(&vario_mps(&times, &altitude));
    build_vario_segments(&buckets, &times, flight.start, flight.end)
        .into_iter()
        .map(|segment| ColorRun {
            color: color_for(segment.bucket),
            // `end_idx` is exclusive; drawing one fix past it joins this run to
            // the next one.
            range: segment.start_idx..(segment.end_idx + 1).min(flight.end),
        })
        .collect()
}

/// Barometric altitude when the track has it — far smoother than GPS (~±0.1 m
/// vs ±2 m). The encoder makes it all-or-nothing per track.
fn altitude_dm(track: &Track) -> Vec<i32> {
    track
        .points
        .iter()
        .map(|point| point.pressure_alt.unwrap_or(point.geo_alt))
        .collect()
}

/// A flat-zero altitude column carries no vario, and neither does one sampled
/// so sparsely that the ±5 s window never sees a neighbour.
fn has_vario_data(times: &[u32], altitude: &[i32], flight: Range<usize>) -> bool {
    let interval = average_fix_interval(times, flight.start, flight.end);
    altitude[flight].iter().any(|&alt| alt != 0) && interval <= MAX_VARIO_FIX_INTERVAL_SECONDS
}

fn color_for(bucket: i8) -> Rgb {
    let idx = (i32::from(bucket) + 5).clamp(0, RAMP.len() as i32 - 1);
    RAMP[idx as usize]
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::sample_track;
    use super::*;
    use tengri_formats::TrackPoint;

    /// 1 Hz fixes: `ground` seconds parked, then `airborne` seconds climbing at
    /// 2 m/s, then `ground` seconds parked again.
    fn track_with_ground(ground: usize, airborne: usize) -> Track {
        let mut points = Vec::new();
        let mut altitude = 10_000;
        for idx in 0..ground * 2 + airborne {
            if (ground..ground + airborne).contains(&idx) {
                altitude += 20;
            }
            points.push(TrackPoint {
                time: idx as u32,
                lat: 42_40000 + idx as i32 * 10,
                lon: 74_00000,
                geo_alt: altitude,
                pressure_alt: None,
                tas: None,
            });
        }
        Track {
            start_time: 0,
            points,
        }
    }

    fn window(takeoff_idx: usize, landing_idx: usize) -> Option<FlightWindow> {
        Some(FlightWindow {
            takeoff_idx,
            landing_idx,
        })
    }

    #[test]
    fn the_ground_segments_are_grey() {
        let track = track_with_ground(60, 120);

        let runs = runs(&track, window(60, 180));

        assert_eq!(runs.first().unwrap().color, GROUND);
        assert_eq!(runs.last().unwrap().color, GROUND);
        assert!(
            runs[1..runs.len() - 1]
                .iter()
                .all(|run| run.color != GROUND)
        );
    }

    #[test]
    fn a_steady_climb_takes_the_warm_end_of_the_ramp() {
        let track = track_with_ground(0, 300);

        let runs = runs(&track, None);

        // +2 m/s the whole way: bucket 2, the client's amber-500.
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].color, RAMP[7]);
    }

    #[test]
    fn a_flight_without_altitude_falls_back_to_blue() {
        let mut track = track_with_ground(0, 120);
        for point in &mut track.points {
            point.geo_alt = 0;
        }

        let runs = runs(&track, None);

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].color, MISSING_ALTITUDE);
    }

    #[test]
    fn a_sparsely_logged_flight_falls_back_to_blue() {
        let mut track = track_with_ground(0, 120);
        for (idx, point) in track.points.iter_mut().enumerate() {
            point.time = idx as u32 * 30;
        }

        let runs = runs(&track, None);

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].color, MISSING_ALTITUDE);
    }

    #[test]
    fn the_runs_cover_the_track_without_a_gap() {
        let track = track_with_ground(60, 300);

        let runs = runs(&track, window(60, 360));

        assert_eq!(runs.first().unwrap().range.start, 0);
        assert_eq!(runs.last().unwrap().range.end, track.points.len());
        for pair in runs.windows(2) {
            assert_eq!(pair[0].range.end, pair[1].range.start + 1);
        }
    }

    #[test]
    fn a_level_track_is_one_neutral_run() {
        let runs = runs(&sample_track(), None);

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].color, RAMP[5]);
    }
}
