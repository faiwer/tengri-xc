//! Integration tests for the compact codec in isolation. The IGC parser is
//! deliberately NOT involved — these tests build `Track`s by hand and verify
//! that `encode → decode` is the identity at the format's documented
//! precision. Pipeline tests (IGC text → Track → CompactTrack → Track) live
//! in dedicated per-format files (e.g. `flight_igc.rs`).

use tengri_server::flight::compact::{CompactTrack, TrackBody};
use tengri_server::flight::{Track, TrackPoint, decode, encode};

fn pt(time: u32, lat: i32, lon: i32, geo_alt: i32, pressure_alt: Option<i32>) -> TrackPoint {
    TrackPoint {
        time,
        lat,
        lon,
        geo_alt,
        pressure_alt,
    }
}

#[test]
fn rejects_empty_track() {
    let track = Track {
        start_time: 0,
        points: vec![],
    };
    assert!(encode(&track).is_err());
}

#[test]
fn rejects_inconsistent_pressure() {
    let track = Track {
        start_time: 0,
        points: vec![pt(0, 0, 0, 0, Some(1000)), pt(1, 1, 1, 1, None)],
    };
    assert!(encode(&track).is_err());
}

#[test]
fn dual_round_trip_simple() {
    // Steady 1 Hz, small deltas, both altitudes.
    let mut points = Vec::new();
    for i in 0u32..10 {
        let t = 1_000_000 + i;
        points.push(pt(
            t,
            4_677_248 + (i as i32) * 3,
            1_314_815 + (i as i32) * 2,
            17_350 + (i as i32) * 5,
            Some(16_640 + (i as i32) * 5),
        ));
    }
    let track = Track {
        start_time: points[0].time,
        points,
    };
    let compact = encode(&track).unwrap();

    assert_eq!(compact.interval, 1);
    match &compact.track {
        TrackBody::Dual { fixes, coords } => {
            assert_eq!(fixes.len(), 1, "only initial fix expected");
            assert_eq!(coords.len(), 9);
        }
        other => panic!("expected Dual body, got {other:?}"),
    }
    assert!(compact.time_fixes.is_empty(), "no time gaps");

    let decoded = decode(&compact).unwrap();
    assert_eq!(decoded, track, "lossless round-trip");
}

#[test]
fn gps_only_round_trip() {
    let points = (0u32..5)
        .map(|i| {
            pt(
                2_000 + i,
                100 + i as i32,
                200 + i as i32,
                1_000 + i as i32,
                None,
            )
        })
        .collect::<Vec<_>>();
    let track = Track {
        start_time: 2_000,
        points,
    };
    let compact = encode(&track).unwrap();
    matches!(compact.track, TrackBody::Gps { .. });
    let decoded = decode(&compact).unwrap();
    assert_eq!(decoded, track);
}

#[test]
fn gps_jump_emits_extra_fix() {
    // The 3rd point (idx=2) jumps 5 000 lat units (>> i8) — should produce
    // an extra fix entry. Reconstruction must still be exact.
    let points = vec![
        pt(0, 4_677_000, 1_314_000, 17_000, Some(16_000)),
        pt(1, 4_677_010, 1_314_010, 17_010, Some(16_010)),
        pt(2, 4_682_000, 1_319_000, 17_020, Some(16_020)),
        pt(3, 4_682_010, 1_319_010, 17_030, Some(16_030)),
    ];
    let track = Track {
        start_time: 0,
        points,
    };
    let compact = encode(&track).unwrap();
    match &compact.track {
        TrackBody::Dual { fixes, coords } => {
            assert_eq!(fixes.len(), 2, "initial + jump");
            assert_eq!(fixes[1].idx, 2);
            assert_eq!(coords.len(), 2);
        }
        _ => panic!("expected Dual"),
    }
    assert_eq!(decode(&compact).unwrap(), track);
}

#[test]
fn altitude_overflow_emits_fix() {
    // 200 dm geo_alt jump (= 20 m in 1 s) — i8 overflow.
    let points = vec![
        pt(0, 0, 0, 17_000, Some(16_000)),
        pt(1, 0, 0, 17_010, Some(16_010)),
        pt(2, 0, 0, 17_220, Some(16_020)), // Δ geo_alt = 210 dm > 127
        pt(3, 0, 0, 17_230, Some(16_030)),
    ];
    let track = Track {
        start_time: 0,
        points,
    };
    let compact = encode(&track).unwrap();
    match &compact.track {
        TrackBody::Dual { fixes, .. } => {
            assert_eq!(fixes.len(), 2, "initial + overflow");
            assert_eq!(fixes[1].idx, 2);
        }
        _ => panic!("expected Dual"),
    }
    assert_eq!(decode(&compact).unwrap(), track);
}

#[test]
fn time_gap_emits_time_fix() {
    // 4-second gap in an otherwise 1 Hz file.
    let points = vec![
        pt(100, 0, 0, 0, None),
        pt(101, 1, 1, 1, None),
        pt(105, 2, 2, 2, None), // 4-s gap
        pt(106, 3, 3, 3, None),
    ];
    let track = Track {
        start_time: 100,
        points,
    };
    let compact = encode(&track).unwrap();
    assert_eq!(compact.interval, 1);
    assert_eq!(compact.time_fixes.len(), 1);
    assert_eq!(compact.time_fixes[0].idx, 2);
    assert_eq!(compact.time_fixes[0].time, 105);

    assert_eq!(decode(&compact).unwrap(), track);
}

#[test]
fn variable_interval_two_hz() {
    // Half-Hz recorder: every fix is 2 s apart.
    let points: Vec<_> = (0u32..6)
        .map(|i| pt(1_000 + i * 2, i as i32, i as i32 * 2, 0, None))
        .collect();
    let track = Track {
        start_time: 1_000,
        points,
    };
    let compact = encode(&track).unwrap();
    assert_eq!(compact.interval, 2);
    assert!(compact.time_fixes.is_empty());
    assert_eq!(decode(&compact).unwrap(), track);
}

/// Deterministic 64-bit xorshift, inlined to keep test deps zero.
struct Rng(u64);
impl Rng {
    fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 as u32
    }
    /// Signed value in `[-range, range]`.
    fn jitter(&mut self, range: i32) -> i32 {
        let span = (range * 2 + 1) as u32;
        (self.next_u32() % span) as i32 - range
    }
}

#[test]
fn round_trip_preserves_dual_track_with_all_encoder_branches() {
    // 500-point track that touches every encoder branch: small deltas
    // (i8 path), big lat/lon jumps (Fix path), big alt jumps (Fix path),
    // and time gaps (TimeFix path). Deterministic via a fixed-seed PRNG so
    // failures reproduce identically.
    let mut rng = Rng(0xDEAD_BEEF_CAFEu64);

    let mut points = Vec::with_capacity(500);
    let mut t: u32 = 1_700_000_000;
    let mut lat: i32 = 4_677_248;
    let mut lon: i32 = 1_314_815;
    let mut geo_alt: i32 = 17_350;
    let mut pressure_alt: i32 = 16_640;

    for i in 0..500 {
        // Per-axis jitter inside i8 range — the dense-delta common case.
        lat += rng.jitter(80);
        lon += rng.jitter(80);
        geo_alt += rng.jitter(20);
        pressure_alt += rng.jitter(20);

        // Sprinkle every kind of overflow into known indices.
        if i == 100 {
            lat += 50_000; // ~550 m GPS jump
        }
        if i == 200 {
            lon -= 30_000;
        }
        if i == 300 {
            geo_alt += 5_000; // 500 m altitude overflow vs i8
            pressure_alt += 5_000;
        }

        // Mostly 1 Hz, with two outsized time gaps.
        let dt = match i {
            150 => 7,
            350 => 12,
            _ => 1,
        };
        t = t.checked_add(dt).unwrap();

        points.push(pt(t, lat, lon, geo_alt, Some(pressure_alt)));
    }

    let track = Track {
        start_time: points[0].time,
        points,
    };

    let compact = encode(&track).expect("encode");
    let decoded = decode(&compact).expect("decode");
    assert_eq!(decoded, track, "encode → decode must be lossless");

    // Sanity: every encoder branch was actually exercised.
    match &compact.track {
        TrackBody::Dual { fixes, coords } => {
            assert!(fixes.len() >= 2, "must have initial fix + overflow fixes");
            assert!(!coords.is_empty());
        }
        other => panic!("expected Dual body, got {other:?}"),
    }
    assert!(
        !compact.time_fixes.is_empty(),
        "time gaps must produce time fixes"
    );

    // And it survives bincode round-trip too.
    let cfg = bincode::config::standard();
    let bytes = bincode::serde::encode_to_vec(&compact, cfg).unwrap();
    let (back, _): (CompactTrack, _) = bincode::serde::decode_from_slice(&bytes, cfg).unwrap();
    assert_eq!(decode(&back).unwrap(), track, "bincode round-trip lossless");
}

#[test]
fn bincode_serialize_round_trip() {
    // Verify CompactTrack survives byte-level serialization (the format
    // we'll actually persist to disk / send over HTTP).
    let track = Track {
        start_time: 0,
        points: vec![
            pt(0, 0, 0, 1_000, Some(900)),
            pt(1, 5, 3, 1_005, Some(902)),
            pt(2, 10, 6, 1_011, Some(904)),
        ],
    };
    let compact = encode(&track).unwrap();
    let cfg = bincode::config::standard();
    let bytes = bincode::serde::encode_to_vec(&compact, cfg).unwrap();
    let (back, _): (CompactTrack, _) = bincode::serde::decode_from_slice(&bytes, cfg).unwrap();
    assert_eq!(compact, back);
    assert_eq!(decode(&back).unwrap(), track);
}
