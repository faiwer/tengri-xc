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
use super::types::{CompactTrack, CoordDual, CoordGps, FixDual, FixGps, TimeFix, TrackBody};

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

    let interval = median_interval(track);
    let start_time = track.points[0].time;

    let body = if has_pressure {
        encode_dual(track)
    } else {
        encode_gps(track)
    };

    let time_fixes = encode_time_fixes(track, start_time, interval);
    let hash = compute_hash(start_time, interval, &body, &time_fixes);

    Ok(CompactTrack {
        start_time,
        interval,
        track: body,
        time_fixes,
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
