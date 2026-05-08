//! FNV-1a 32 over a [`CompactTrack`]'s payload (everything except the `hash`
//! field). Stored alongside the data so the client can verify the wire
//! payload survived transport and that both parsers agree on the shape.
//!
//! Why FNV-1a 32?
//! - Trivial to implement on both sides (no extra deps on the client).
//! - Plenty for corruption / version-skew detection. Not a cryptographic
//!   hash; don't use it for anything that needs collision resistance.
//!
//! Wire ordering MUST match `client/src/track/decode/hash.ts`. Both sides
//! walk the fields in declaration order and feed primitives as little-endian
//! bytes. Any change here is a wire break — bump
//! [`crate::flight::tengri::VERSION`] and update the TS side in lockstep.

use super::types::{CoordDual, CoordGps, FixDual, FixGps, TasBody, TasFix, TimeFix, TrackBody};

const FNV_OFFSET: u32 = 0x811c_9dc5;
const FNV_PRIME: u32 = 0x0100_0193;

/// Compute the hash of the `CompactTrack` payload (every field except `hash`
/// itself).
pub fn compute(
    start_time: u32,
    interval: u16,
    track: &TrackBody,
    time_fixes: &[TimeFix],
    tas: &TasBody,
) -> u32 {
    let mut h = FNV_OFFSET;
    feed_u32(&mut h, start_time);
    feed_u16(&mut h, interval);
    feed_track(&mut h, track);
    feed_time_fixes(&mut h, time_fixes);
    feed_tas(&mut h, tas);
    h
}

fn feed_byte(h: &mut u32, b: u8) {
    *h ^= b as u32;
    *h = h.wrapping_mul(FNV_PRIME);
}

fn feed_u16(h: &mut u32, v: u16) {
    for b in v.to_le_bytes() {
        feed_byte(h, b);
    }
}

fn feed_u32(h: &mut u32, v: u32) {
    for b in v.to_le_bytes() {
        feed_byte(h, b);
    }
}

fn feed_i32(h: &mut u32, v: i32) {
    for b in v.to_le_bytes() {
        feed_byte(h, b);
    }
}

fn feed_track(h: &mut u32, track: &TrackBody) {
    match track {
        TrackBody::Gps { fixes, coords } => {
            feed_byte(h, 0);
            feed_u32(h, fixes.len() as u32);
            for f in fixes {
                feed_fix_gps(h, f);
            }
            feed_u32(h, coords.len() as u32);
            for c in coords {
                feed_coord_gps(h, c);
            }
        }
        TrackBody::Dual { fixes, coords } => {
            feed_byte(h, 1);
            feed_u32(h, fixes.len() as u32);
            for f in fixes {
                feed_fix_dual(h, f);
            }
            feed_u32(h, coords.len() as u32);
            for c in coords {
                feed_coord_dual(h, c);
            }
        }
    }
}

fn feed_fix_gps(h: &mut u32, f: &FixGps) {
    feed_u32(h, f.idx);
    feed_i32(h, f.lat);
    feed_i32(h, f.lon);
    feed_i32(h, f.geo_alt);
}

fn feed_fix_dual(h: &mut u32, f: &FixDual) {
    feed_u32(h, f.idx);
    feed_i32(h, f.lat);
    feed_i32(h, f.lon);
    feed_i32(h, f.geo_alt);
    feed_i32(h, f.pressure_alt);
}

fn feed_coord_gps(h: &mut u32, c: &CoordGps) {
    feed_byte(h, c.lat as u8);
    feed_byte(h, c.lon as u8);
    feed_byte(h, c.geo_alt as u8);
}

fn feed_coord_dual(h: &mut u32, c: &CoordDual) {
    feed_byte(h, c.lat as u8);
    feed_byte(h, c.lon as u8);
    feed_byte(h, c.geo_alt as u8);
    feed_byte(h, c.pressure_alt as u8);
}

fn feed_time_fixes(h: &mut u32, tf: &[TimeFix]) {
    feed_u32(h, tf.len() as u32);
    for t in tf {
        feed_u32(h, t.idx);
        feed_u32(h, t.time);
    }
}

fn feed_tas(h: &mut u32, tas: &TasBody) {
    match tas {
        TasBody::None => feed_byte(h, 0),
        TasBody::Tas { fixes, deltas } => {
            feed_byte(h, 1);
            feed_u32(h, fixes.len() as u32);
            for f in fixes {
                feed_tas_fix(h, f);
            }
            feed_u32(h, deltas.len() as u32);
            for d in deltas {
                feed_byte(h, *d as u8);
            }
        }
    }
}

fn feed_tas_fix(h: &mut u32, f: &TasFix) {
    feed_u32(h, f.idx);
    feed_u16(h, f.tas);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FNV-1a 32 of ASCII "foobar" → 0xbf9cf968 (canonical isthe.com test
    /// vector). Anchors the algorithm; if this fails, we've broken the
    /// constants or the byte-feeding loop, not the wire-format ordering.
    #[test]
    fn fnv1a32_canonical_test_vector() {
        let mut h = FNV_OFFSET;
        for b in b"foobar" {
            feed_byte(&mut h, *b);
        }
        assert_eq!(h, 0xbf9c_f968);
    }

    /// Deterministic on identical input, sensitive to a single-field change.
    #[test]
    fn detects_single_field_change() {
        let track = TrackBody::Gps {
            fixes: vec![FixGps {
                idx: 0,
                lat: 1,
                lon: 2,
                geo_alt: 3,
            }],
            coords: vec![],
        };
        let h1 = compute(100, 1, &track, &[], &TasBody::None);
        let h2 = compute(100, 1, &track, &[], &TasBody::None);
        assert_eq!(h1, h2);

        let h3 = compute(101, 1, &track, &[], &TasBody::None);
        assert_ne!(h1, h3);
    }

    /// Hash distinguishes presence of TAS even when the rest is identical.
    /// Catches a regression where the FE and BE disagree on whether the
    /// flight has an airspeed channel.
    #[test]
    fn detects_tas_presence_change() {
        let track = TrackBody::Gps {
            fixes: vec![FixGps {
                idx: 0,
                lat: 1,
                lon: 2,
                geo_alt: 3,
            }],
            coords: vec![],
        };
        let no_tas = compute(100, 1, &track, &[], &TasBody::None);
        let with_tas = compute(
            100,
            1,
            &track,
            &[],
            &TasBody::Tas {
                fixes: vec![TasFix { idx: 0, tas: 50 }],
                deltas: vec![],
            },
        );
        assert_ne!(no_tas, with_tas);
    }
}
