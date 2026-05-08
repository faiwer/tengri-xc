//! Decoder: `CompactTrack` → in-memory `Track`.
//!
//! Two-cursor walk:
//!
//! - At each index `i ∈ 0..N`, if the next entry in `fixes` matches `i`,
//!   adopt it as the absolute state. Otherwise, apply the next entry from
//!   `coords` to the running state. (`coords` has no entry at fix indices.)
//! - For time: walk a third cursor over `time_fixes`. The most recent
//!   override (or the initial `start_time` at idx 0) plus
//!   `(i − anchor_idx) × interval` gives the timestamp.

use crate::flight::types::{Track, TrackPoint};

use super::error::CompactError;
use super::types::{
    CompactTrack, CoordDual, CoordGps, FixDual, FixGps, TasBody, TimeFix, TrackBody,
};

pub fn decode(compact: &CompactTrack) -> Result<Track, CompactError> {
    validate(compact)?;

    let total = compact.len();
    let mut points: Vec<TrackPoint> = Vec::with_capacity(total);

    let mut anchor_idx: u32 = 0;
    let mut anchor_time: u32 = compact.start_time;
    let mut t_cur: usize = 0;
    let interval = compact.interval as u32;

    match &compact.track {
        TrackBody::Gps { fixes, coords } => {
            decode_loop_gps(
                fixes,
                coords,
                total,
                &mut points,
                &mut t_cur,
                &mut anchor_idx,
                &mut anchor_time,
                interval,
                &compact.time_fixes,
            )?;
        }
        TrackBody::Dual { fixes, coords } => {
            decode_loop_dual(
                fixes,
                coords,
                total,
                &mut points,
                &mut t_cur,
                &mut anchor_idx,
                &mut anchor_time,
                interval,
                &compact.time_fixes,
            )?;
        }
    }

    apply_tas(&mut points, &compact.tas)?;

    let start_time = compact.start_time;

    Ok(Track { start_time, points })
}

/// Walk `TasBody` in lockstep with `points`, stamping `tas: Some(u16)`
/// on each. `TasBody::None` is a no-op (every `TrackPoint.tas` already
/// defaults to `None` from the position decoder loops above).
fn apply_tas(points: &mut [TrackPoint], tas: &TasBody) -> Result<(), CompactError> {
    let TasBody::Tas { fixes, deltas } = tas else {
        return Ok(());
    };
    if fixes.len() + deltas.len() != points.len() {
        return Err(CompactError::IndexOutOfRange {
            idx: (fixes.len() + deltas.len()) as u32,
            len: points.len() as u32,
        });
    }
    if fixes.is_empty() || fixes[0].idx != 0 {
        return Err(CompactError::MissingInitialFix);
    }
    for w in fixes.windows(2) {
        if w[1].idx <= w[0].idx {
            return Err(CompactError::UnorderedFixes {
                prev: w[0].idx,
                next: w[1].idx,
            });
        }
    }

    let mut state: u16 = fixes[0].tas;
    let mut fix_cur: usize = 1;
    let mut delta_cur: usize = 0;
    points[0].tas = Some(state);

    for (i, p) in points.iter_mut().enumerate().skip(1) {
        let idx = i as u32;
        if fix_cur < fixes.len() && fixes[fix_cur].idx == idx {
            state = fixes[fix_cur].tas;
            fix_cur += 1;
        } else {
            // Saturating add via i32 keeps the math obvious; legitimate
            // values cannot wrap (encoder enforces fix overrides).
            let next = state as i32 + deltas[delta_cur] as i32;
            // Clamp to u16 range for safety; in practice valid encoded
            // streams never trigger this.
            state = next.clamp(0, u16::MAX as i32) as u16;
            delta_cur += 1;
        }
        p.tas = Some(state);
    }
    Ok(())
}

fn validate(compact: &CompactTrack) -> Result<(), CompactError> {
    let total = compact.len();
    if total == 0 {
        return Err(CompactError::EmptyTrack);
    }
    let total_u32 = total as u32;

    let (fix_indices, fixes_len, coords_len): (Vec<u32>, usize, usize) = match &compact.track {
        TrackBody::Gps { fixes, coords } => (
            fixes.iter().map(|f| f.idx).collect(),
            fixes.len(),
            coords.len(),
        ),
        TrackBody::Dual { fixes, coords } => (
            fixes.iter().map(|f| f.idx).collect(),
            fixes.len(),
            coords.len(),
        ),
    };

    if fixes_len == 0 || fix_indices[0] != 0 {
        return Err(CompactError::MissingInitialFix);
    }
    for w in fix_indices.windows(2) {
        if w[1] <= w[0] {
            return Err(CompactError::UnorderedFixes {
                prev: w[0],
                next: w[1],
            });
        }
    }
    if let Some(&last) = fix_indices.last()
        && last >= total_u32
    {
        return Err(CompactError::IndexOutOfRange {
            idx: last,
            len: total_u32,
        });
    }
    let _ = coords_len;

    let tf = &compact.time_fixes;
    for w in tf.windows(2) {
        if w[1].idx <= w[0].idx {
            return Err(CompactError::UnorderedTimeFixes);
        }
    }
    if let Some(last) = tf.last()
        && last.idx >= total_u32
    {
        return Err(CompactError::IndexOutOfRange {
            idx: last.idx,
            len: total_u32,
        });
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn decode_loop_gps(
    fixes: &[FixGps],
    coords: &[CoordGps],
    total: usize,
    out: &mut Vec<TrackPoint>,
    t_cur: &mut usize,
    anchor_idx: &mut u32,
    anchor_time: &mut u32,
    interval: u32,
    time_fixes: &[TimeFix],
) -> Result<(), CompactError> {
    let mut coord_cur: usize = 0;
    // Initial state from fixes[0] (validated to exist at idx=0).
    let f0 = &fixes[0];
    let (mut s_lat, mut s_lon, mut s_galt) = (f0.lat, f0.lon, f0.geo_alt);
    let mut fix_cur: usize = 1;

    for i in 0..total {
        let idx = i as u32;
        if i > 0 {
            if fix_cur < fixes.len() && fixes[fix_cur].idx == idx {
                let f = &fixes[fix_cur];
                s_lat = f.lat;
                s_lon = f.lon;
                s_galt = f.geo_alt;
                fix_cur += 1;
            } else {
                if coord_cur >= coords.len() {
                    return Err(CompactError::IndexOutOfRange {
                        idx,
                        len: total as u32,
                    });
                }
                let c = &coords[coord_cur];
                s_lat += c.lat as i32;
                s_lon += c.lon as i32;
                s_galt += c.geo_alt as i32;
                coord_cur += 1;
            }
        }

        let time = resolve_time(idx, t_cur, anchor_idx, anchor_time, interval, time_fixes);
        out.push(TrackPoint {
            time,
            lat: s_lat,
            lon: s_lon,
            geo_alt: s_galt,
            pressure_alt: None,
            tas: None,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn decode_loop_dual(
    fixes: &[FixDual],
    coords: &[CoordDual],
    total: usize,
    out: &mut Vec<TrackPoint>,
    t_cur: &mut usize,
    anchor_idx: &mut u32,
    anchor_time: &mut u32,
    interval: u32,
    time_fixes: &[TimeFix],
) -> Result<(), CompactError> {
    let mut coord_cur: usize = 0;
    let f0 = &fixes[0];
    let (mut s_lat, mut s_lon, mut s_galt, mut s_palt) =
        (f0.lat, f0.lon, f0.geo_alt, f0.pressure_alt);
    let mut fix_cur: usize = 1;

    for i in 0..total {
        let idx = i as u32;
        if i > 0 {
            if fix_cur < fixes.len() && fixes[fix_cur].idx == idx {
                let f = &fixes[fix_cur];
                s_lat = f.lat;
                s_lon = f.lon;
                s_galt = f.geo_alt;
                s_palt = f.pressure_alt;
                fix_cur += 1;
            } else {
                if coord_cur >= coords.len() {
                    return Err(CompactError::IndexOutOfRange {
                        idx,
                        len: total as u32,
                    });
                }
                let c = &coords[coord_cur];
                s_lat += c.lat as i32;
                s_lon += c.lon as i32;
                s_galt += c.geo_alt as i32;
                s_palt += c.pressure_alt as i32;
                coord_cur += 1;
            }
        }

        let time = resolve_time(idx, t_cur, anchor_idx, anchor_time, interval, time_fixes);
        out.push(TrackPoint {
            time,
            lat: s_lat,
            lon: s_lon,
            geo_alt: s_galt,
            pressure_alt: Some(s_palt),
            tas: None,
        });
    }
    Ok(())
}

fn resolve_time(
    idx: u32,
    t_cur: &mut usize,
    anchor_idx: &mut u32,
    anchor_time: &mut u32,
    interval: u32,
    time_fixes: &[TimeFix],
) -> u32 {
    if *t_cur < time_fixes.len() && time_fixes[*t_cur].idx == idx {
        *anchor_idx = idx;
        *anchor_time = time_fixes[*t_cur].time;
        *t_cur += 1;
    }
    *anchor_time + (idx - *anchor_idx) * interval
}
