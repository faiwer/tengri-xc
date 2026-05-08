//! Encoder: in-memory `Track` → `CompactTrack`.
//!
//! Pseudocode:
//!
//! 1. Reject empty tracks; reject mixed-pressure tracks.
//! 2. Compute median Δt across consecutive points → `interval`.
//! 3. Walk points. The first one is emitted as a `Fix*` (mandatory).
//!    For subsequent points, compute (Δlat, Δlon, Δgeo_alt, [Δpressure_alt])
//!    from the previously-encoded state. If any of those overflow `i8`,
//!    emit a `Fix*` (absolute reset). Otherwise emit a `Coord*`.
//! 4. For each point's actual time, compare against the anchor-based
//!    expectation `anchor_time + (i − anchor_idx) × interval`. On
//!    mismatch, emit a `TimeFix(i, actual_time)` and reset the anchor.

use crate::flight::types::Track;

use super::error::CompactError;
use super::hash::compute as compute_hash;
use super::types::{
    CompactTrack, CoordDual, CoordGps, FixDual, FixGps, TasBody, TasFix, TimeFix, TrackBody,
};

pub fn encode(track: &Track) -> Result<CompactTrack, CompactError> {
    if track.points.is_empty() {
        return Err(CompactError::EmptyTrack);
    }

    let has_pressure = track.points[0].pressure_alt.is_some();
    if track
        .points
        .iter()
        .any(|p| p.pressure_alt.is_some() != has_pressure)
    {
        return Err(CompactError::InconsistentPressureAlt);
    }

    let has_tas = track.points[0].tas.is_some();
    if track.points.iter().any(|p| p.tas.is_some() != has_tas) {
        return Err(CompactError::InconsistentTas);
    }

    let interval = median_interval(track);
    let start_time = track.points[0].time;

    let body = if has_pressure {
        encode_dual(track)
    } else {
        encode_gps(track)
    };

    let time_fixes = encode_time_fixes(track, start_time, interval);
    let tas = if has_tas {
        encode_tas(track)
    } else {
        TasBody::None
    };
    let hash = compute_hash(start_time, interval, &body, &time_fixes, &tas);

    Ok(CompactTrack {
        start_time,
        interval,
        track: body,
        time_fixes,
        tas,
        hash,
    })
}

fn median_interval(track: &Track) -> u16 {
    if track.points.len() < 2 {
        return 1;
    }
    let mut deltas: Vec<u32> = track
        .points
        .windows(2)
        .map(|w| w[1].time.saturating_sub(w[0].time))
        .collect();
    deltas.sort_unstable();
    let median = deltas[deltas.len() / 2].max(1);
    // Clamp to u16. Realistic samples are 1..60 s; bigger values would
    // still be valid but they signal a degenerate file.
    median.min(u16::MAX as u32) as u16
}

fn fits_i8(v: i32) -> bool {
    (i8::MIN as i32..=i8::MAX as i32).contains(&v)
}

fn encode_gps(track: &Track) -> TrackBody {
    let n = track.points.len();
    let mut fixes: Vec<FixGps> = Vec::with_capacity(8);
    let mut coords: Vec<CoordGps> = Vec::with_capacity(n);

    let p0 = &track.points[0];
    fixes.push(FixGps {
        idx: 0,
        lat: p0.lat,
        lon: p0.lon,
        geo_alt: p0.geo_alt,
    });
    let (mut s_lat, mut s_lon, mut s_galt) = (p0.lat, p0.lon, p0.geo_alt);

    for (i, p) in track.points.iter().enumerate().skip(1) {
        let idx = i as u32;
        let dlat = p.lat - s_lat;
        let dlon = p.lon - s_lon;
        let dgalt = p.geo_alt - s_galt;

        if fits_i8(dlat) && fits_i8(dlon) && fits_i8(dgalt) {
            coords.push(CoordGps {
                lat: dlat as i8,
                lon: dlon as i8,
                geo_alt: dgalt as i8,
            });
        } else {
            fixes.push(FixGps {
                idx,
                lat: p.lat,
                lon: p.lon,
                geo_alt: p.geo_alt,
            });
        }
        s_lat = p.lat;
        s_lon = p.lon;
        s_galt = p.geo_alt;
    }

    TrackBody::Gps { fixes, coords }
}

fn encode_dual(track: &Track) -> TrackBody {
    let n = track.points.len();
    let mut fixes: Vec<FixDual> = Vec::with_capacity(8);
    let mut coords: Vec<CoordDual> = Vec::with_capacity(n);

    let p0 = &track.points[0];
    let palt0 = p0
        .pressure_alt
        .expect("dual track point must carry pressure_alt");
    fixes.push(FixDual {
        idx: 0,
        lat: p0.lat,
        lon: p0.lon,
        geo_alt: p0.geo_alt,
        pressure_alt: palt0,
    });
    let (mut s_lat, mut s_lon, mut s_galt, mut s_palt) = (p0.lat, p0.lon, p0.geo_alt, palt0);

    for (i, p) in track.points.iter().enumerate().skip(1) {
        let idx = i as u32;
        let palt = p
            .pressure_alt
            .expect("dual track point must carry pressure_alt");
        let dlat = p.lat - s_lat;
        let dlon = p.lon - s_lon;
        let dgalt = p.geo_alt - s_galt;
        let dpalt = palt - s_palt;

        if fits_i8(dlat) && fits_i8(dlon) && fits_i8(dgalt) && fits_i8(dpalt) {
            coords.push(CoordDual {
                lat: dlat as i8,
                lon: dlon as i8,
                geo_alt: dgalt as i8,
                pressure_alt: dpalt as i8,
            });
        } else {
            fixes.push(FixDual {
                idx,
                lat: p.lat,
                lon: p.lon,
                geo_alt: p.geo_alt,
                pressure_alt: palt,
            });
        }
        s_lat = p.lat;
        s_lon = p.lon;
        s_galt = p.geo_alt;
        s_palt = palt;
    }

    TrackBody::Dual { fixes, coords }
}

/// Encode the per-point TAS sequence as `fixes + deltas`. Caller has
/// already verified that every `TrackPoint.tas` is `Some`.
///
/// Strategy mirrors `encode_dual`: walk the points, push the first one
/// as an absolute fix at `idx=0`, and for each subsequent point either
/// emit an `i8` delta or — when the delta would overflow — an absolute
/// fix override. On hang gliders and paragliders the override path is
/// effectively never taken (worst observed |Δtas| is ~50 km/h on
/// takeoff/landing transitions, well within `i8::MAX`). Sailplanes
/// regularly exceed `i8` deltas — VNE ~285 km/h, rapid pull-ups, and
/// final-glide pushes routinely produce |Δ| > 127 — and each such
/// event costs one extra `TasFix` (~6 bytes), which is negligible.
fn encode_tas(track: &Track) -> TasBody {
    let n = track.points.len();
    let mut fixes: Vec<TasFix> = Vec::with_capacity(1);
    let mut deltas: Vec<i8> = Vec::with_capacity(n);

    let tas0 = track.points[0]
        .tas
        .expect("encode_tas requires every point to carry tas");
    fixes.push(TasFix { idx: 0, tas: tas0 });
    let mut state: u16 = tas0;

    for (i, p) in track.points.iter().enumerate().skip(1) {
        let tas = p.tas.expect("encode_tas requires every point to carry tas");
        let delta = tas as i32 - state as i32;
        if fits_i8(delta) {
            deltas.push(delta as i8);
        } else {
            fixes.push(TasFix { idx: i as u32, tas });
        }
        state = tas;
    }

    TasBody::Tas { fixes, deltas }
}

fn encode_time_fixes(track: &Track, start_time: u32, interval: u16) -> Vec<TimeFix> {
    let mut out: Vec<TimeFix> = Vec::new();
    let mut anchor_idx: u32 = 0;
    let mut anchor_time: u32 = start_time;
    let interval = interval as u32;

    for (i, p) in track.points.iter().enumerate().skip(1) {
        let idx = i as u32;
        let expected = anchor_time + (idx - anchor_idx) * interval;
        if p.time != expected {
            out.push(TimeFix { idx, time: p.time });
            anchor_idx = idx;
            anchor_time = p.time;
        }
    }

    out
}
