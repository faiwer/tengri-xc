//! Tiny helpers shared by text-based track parsers (KML, GPX).
//!
//! These are deliberately error-type-agnostic: they return plain
//! `Result<_, String>` so each parser can wrap the message into its
//! own error variant without dragging parser-specific types into a
//! "common" module.

use chrono::{DateTime, Utc};

/// Decimal degrees → E5 micro-degrees, rounded half-away-from-zero.
/// Source resolution from typical GPS recorders is 5–7 decimal digits;
/// E5 (~1.11 m at the equator) preserves all useful precision.
pub fn deg_to_e5(deg: f64) -> i32 {
    let scaled = deg * 100_000.0;
    if scaled >= 0.0 {
        (scaled + 0.5).floor() as i32
    } else {
        (scaled - 0.5).ceil() as i32
    }
}

/// Metres → decimeters, rounded. Track altitudes are stored as
/// decimeters everywhere downstream.
pub fn m_to_dm(m: f64) -> i32 {
    (m * 10.0).round() as i32
}

/// Parse an RFC 3339 / ISO 8601 timestamp and bound-check to `u32`
/// epoch seconds. Used by every track parser that targets the
/// `TrackPoint::time` field.
pub fn parse_iso8601_u32(s: &str) -> Result<u32, String> {
    let dt = DateTime::parse_from_rfc3339(s.trim())
        .map_err(|e| format!("expected RFC 3339 / ISO 8601 timestamp, got {s:?}: {e}"))?;
    let secs = dt.with_timezone(&Utc).timestamp();
    if !(0..=u32::MAX as i64).contains(&secs) {
        return Err(format!("timestamp {s:?} is outside the u32-epoch range"));
    }
    Ok(secs as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deg_to_e5_rounds_half_away_from_zero() {
        assert_eq!(deg_to_e5(13.166333), 1_316_633);
        assert_eq!(deg_to_e5(-0.000005), -1);
        assert_eq!(deg_to_e5(0.0), 0);
        assert_eq!(deg_to_e5(46.765998), 4_676_600);
    }

    #[test]
    fn m_to_dm_rounds_to_nearest() {
        assert_eq!(m_to_dm(1741.0), 17_410);
        assert_eq!(m_to_dm(1741.4), 17_414);
        assert_eq!(m_to_dm(1741.5), 17_415);
        assert_eq!(m_to_dm(1741.6), 17_416);
        assert_eq!(m_to_dm(-100.5), -1_005);
    }

    #[test]
    fn parse_iso8601_u32_accepts_z_suffix() {
        let secs = parse_iso8601_u32("2026-05-02T10:54:33Z").unwrap();
        let expected = DateTime::parse_from_rfc3339("2026-05-02T10:54:33Z")
            .unwrap()
            .timestamp() as u32;
        assert_eq!(secs, expected);
    }

    #[test]
    fn parse_iso8601_u32_accepts_offset() {
        let z = parse_iso8601_u32("2026-05-02T10:54:33Z").unwrap();
        let plus_two = parse_iso8601_u32("2026-05-02T12:54:33+02:00").unwrap();
        assert_eq!(z, plus_two);
    }

    #[test]
    fn parse_iso8601_u32_rejects_garbage() {
        assert!(parse_iso8601_u32("not-a-date").is_err());
        assert!(parse_iso8601_u32("").is_err());
    }
}
