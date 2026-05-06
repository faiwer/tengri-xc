//! Integration tests for the IGC parser. We construct synthetic IGC text
//! with the helpers in this file and assert on the resulting `Track`.
//! Compact encode/decode is deliberately NOT exercised here — that lives in
//! `flight_compact.rs`.

use std::fmt::Write;

use tengri_server::flight::{IgcError, parse_str};

#[derive(Clone, Copy)]
struct LatPacked {
    deg: u32,
    min_thousandths: u32,
    hemi: char,
}

#[derive(Clone, Copy)]
struct LonPacked {
    deg: u32,
    min_thousandths: u32,
    hemi: char,
}

#[derive(Clone, Copy)]
struct Sample {
    /// Seconds since the track's UTC start.
    seconds_after_start: u32,
    lat: LatPacked,
    lon: LonPacked,
    pressure_alt_m: i32,
    geo_alt_m: i32,
}

/// Build a single B-record line in the IGC byte layout the parser expects.
fn b_record(s: Sample, h0: u32, m0: u32, s0: u32) -> String {
    let total = h0 * 3600 + m0 * 60 + s0 + s.seconds_after_start;
    let h = (total / 3600) % 24;
    let m = (total / 60) % 60;
    let ss = total % 60;
    format!(
        "B{h:02}{m:02}{ss:02}{:02}{:05}{}{:03}{:05}{}A{:05}{:05}",
        s.lat.deg,
        s.lat.min_thousandths,
        s.lat.hemi,
        s.lon.deg,
        s.lon.min_thousandths,
        s.lon.hemi,
        s.pressure_alt_m,
        s.geo_alt_m,
    )
}

fn build_igc(date_ddmmyy: &str, samples: &[Sample], h: u32, m: u32, s: u32) -> String {
    let mut out = String::new();
    writeln!(out, "AXCT000synthetic-test").unwrap();
    writeln!(out, "HFDTEDATE:{date_ddmmyy},01").unwrap();
    writeln!(out, "HFPLTPILOTINCHARGE:Test Pilot").unwrap();
    writeln!(out, "HFGTYGLIDERTYPE:Test Wing").unwrap();
    for sample in samples {
        writeln!(out, "{}", b_record(*sample, h, m, s)).unwrap();
    }
    out
}

/// Build N consecutive 1 Hz samples in the Alps with a steady climb.
fn make_clean_track(n: usize) -> Vec<Sample> {
    (0..n as u32)
        .map(|i| Sample {
            seconds_after_start: i,
            lat: LatPacked {
                deg: 46,
                min_thousandths: 46_349 + i,
                hemi: 'N',
            },
            lon: LonPacked {
                deg: 13,
                min_thousandths: 8_989 + i,
                hemi: 'E',
            },
            pressure_alt_m: 1_664 + i as i32,
            geo_alt_m: 1_735 + i as i32,
        })
        .collect()
}

#[test]
fn parses_clean_track() {
    let samples = make_clean_track(50);
    let igc = build_igc("030526", &samples, 10, 0, 0);

    let track = parse_str(&igc).expect("parse");

    assert_eq!(track.points.len(), 50);
    assert_eq!(track.start_time, track.points[0].time);

    // Pressure altitudes are present for every point and stored in decimeters.
    assert!(track.points.iter().all(|p| p.pressure_alt.is_some()));
    assert_eq!(track.points[0].pressure_alt, Some(16_640));
    assert_eq!(track.points[0].geo_alt, 17_350);
    assert_eq!(track.points[49].pressure_alt, Some(17_130));

    // Steady 1 Hz: timestamps are strictly consecutive seconds.
    for w in track.points.windows(2) {
        assert_eq!(w[1].time, w[0].time + 1);
    }

    // Northern + eastern hemispheres → both axes positive.
    assert!(track.points[0].lat > 0);
    assert!(track.points[0].lon > 0);
}

#[test]
fn parses_southern_and_western_hemispheres() {
    // Patagonia, single fix.
    let samples = [Sample {
        seconds_after_start: 0,
        lat: LatPacked {
            deg: 41,
            min_thousandths: 50_123,
            hemi: 'S',
        },
        lon: LonPacked {
            deg: 71,
            min_thousandths: 15_456,
            hemi: 'W',
        },
        pressure_alt_m: 500,
        geo_alt_m: 510,
    }];

    let track = parse_str(&build_igc("030526", &samples, 12, 0, 0)).expect("parse");

    assert_eq!(track.points.len(), 1);
    assert!(track.points[0].lat < 0, "S hemisphere → negative lat");
    assert!(track.points[0].lon < 0, "W hemisphere → negative lon");
}

#[test]
fn preserves_time_gaps() {
    // 3-second dropout between fix 4 and 5 → Δt = 4 across that boundary.
    let mut samples = make_clean_track(10);
    for s in samples.iter_mut().skip(5) {
        s.seconds_after_start += 3;
    }
    let track = parse_str(&build_igc("030526", &samples, 10, 0, 0)).expect("parse");

    assert_eq!(track.points[5].time - track.points[4].time, 4);
    assert_eq!(track.points[6].time - track.points[5].time, 1);
}

#[test]
fn demotes_all_zero_pressure_to_gps_only() {
    // IGC files from GPS-only loggers fill the pressure column with zeros.
    // The parser must surface this as `pressure_alt = None` for every point.
    let mut samples = make_clean_track(10);
    for s in samples.iter_mut() {
        s.pressure_alt_m = 0;
    }
    let track = parse_str(&build_igc("030526", &samples, 10, 0, 0)).expect("parse");

    assert!(track.points.iter().all(|p| p.pressure_alt.is_none()));
}

#[test]
fn rejects_input_without_b_records() {
    let igc = "AXCT000\nHFDTEDATE:030526,01\nHFPLTPILOTINCHARGE:foo\n";
    let err = parse_str(igc).expect_err("must reject");
    assert!(matches!(err, IgcError::NoFixes), "got {err:?}");
}
