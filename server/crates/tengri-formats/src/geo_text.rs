//! Tiny helpers shared by text-based track parsers (KML, GPX).
//!
//! These are deliberately error-type-agnostic: they return plain
//! `Result<_, String>` so each parser can wrap the message into its
//! own error variant without dragging parser-specific types into a
//! "common" module.

use chrono::{DateTime, NaiveDateTime, Utc};

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
    secs_to_u32(dt.with_timezone(&Utc).timestamp(), s)
}

/// Parse the GPSBabel/OGR-style timestamp emitted as a KML `<SimpleData
/// name="time">` value: `YYYY/MM/DD HH:MM:SS[±HH[MM]|±HH:MM|Z]`. The
/// canonical form is `2026/05/02 10:54:33+00` (a two-digit hours-only
/// offset, no minutes, no colon — that's what `ogr2ogr` writes by
/// default). We accept the common variants the same tooling can produce
/// in case a fork tightens up the format.
pub fn parse_gpsbabel_time_u32(s: &str) -> Result<u32, String> {
    let trimmed = s.trim();

    // Split off the timezone suffix. We need to do this manually because
    // chrono's offset parsers can't agree on shapes like `+00` vs `+0000`
    // vs `+00:00` vs `Z` in one call.
    let (naive_part, offset_seconds) = split_time_offset(trimmed)
        .ok_or_else(|| format!("expected GPSBabel timestamp, got {s:?}"))?;

    let naive = NaiveDateTime::parse_from_str(naive_part, "%Y/%m/%d %H:%M:%S").map_err(|e| {
        format!("expected GPSBabel timestamp 'YYYY/MM/DD HH:MM:SS', got {s:?}: {e}")
    })?;

    let utc_secs = naive.and_utc().timestamp() - offset_seconds;
    secs_to_u32(utc_secs, s)
}

fn split_time_offset(s: &str) -> Option<(&str, i64)> {
    if let Some(stripped) = s.strip_suffix('Z').or_else(|| s.strip_suffix('z')) {
        return Some((stripped.trim_end(), 0));
    }
    let sign_pos = s.rfind(['+', '-'])?;
    if sign_pos == 0 {
        return None;
    }
    let (date_part, off_part) = s.split_at(sign_pos);
    let sign: i64 = if off_part.starts_with('+') { 1 } else { -1 };
    let body = &off_part[1..];

    let (h, m): (i64, i64) = match body.len() {
        2 => (body.parse().ok()?, 0),
        4 => (body[..2].parse().ok()?, body[2..].parse().ok()?),
        5 if body.as_bytes()[2] == b':' => (body[..2].parse().ok()?, body[3..].parse().ok()?),
        _ => return None,
    };
    if !(0..24).contains(&h) || !(0..60).contains(&m) {
        return None;
    }
    Some((date_part.trim_end(), sign * (h * 3600 + m * 60)))
}

fn secs_to_u32(secs: i64, original: &str) -> Result<u32, String> {
    if !(0..=u32::MAX as i64).contains(&secs) {
        return Err(format!(
            "timestamp {original:?} is outside the u32-epoch range"
        ));
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

    #[test]
    fn parse_gpsbabel_time_handles_canonical_two_digit_offset() {
        // GPSBabel/OGR ships hours-only suffix without a colon.
        let secs = parse_gpsbabel_time_u32("2026/05/02 10:54:33+00").unwrap();
        let expected = parse_iso8601_u32("2026-05-02T10:54:33Z").unwrap();
        assert_eq!(secs, expected);
    }

    #[test]
    fn parse_gpsbabel_time_handles_offset_variants() {
        let canon = parse_iso8601_u32("2026-05-02T10:54:33Z").unwrap();
        // +0000 four-digit / +00:00 with colon / Z literal — same instant.
        assert_eq!(
            parse_gpsbabel_time_u32("2026/05/02 10:54:33+0000").unwrap(),
            canon
        );
        assert_eq!(
            parse_gpsbabel_time_u32("2026/05/02 10:54:33+00:00").unwrap(),
            canon
        );
        assert_eq!(
            parse_gpsbabel_time_u32("2026/05/02 10:54:33Z").unwrap(),
            canon
        );
    }

    #[test]
    fn parse_gpsbabel_time_handles_non_utc_offset() {
        let utc = parse_iso8601_u32("2026-05-02T10:54:33Z").unwrap();
        // 12:54:33 in +02:00 is 10:54:33 UTC.
        assert_eq!(
            parse_gpsbabel_time_u32("2026/05/02 12:54:33+02").unwrap(),
            utc
        );
        assert_eq!(
            parse_gpsbabel_time_u32("2026/05/02 08:54:33-02").unwrap(),
            utc
        );
    }

    #[test]
    fn parse_gpsbabel_time_rejects_garbage() {
        assert!(parse_gpsbabel_time_u32("not-a-date").is_err());
        assert!(parse_gpsbabel_time_u32("").is_err());
        assert!(parse_gpsbabel_time_u32("2026-05-02T10:54:33Z").is_err()); // ISO, not GPSBabel
        assert!(parse_gpsbabel_time_u32("2026/05/02 10:54:33").is_err()); // missing offset
        assert!(parse_gpsbabel_time_u32("2026/05/02 10:54:33+99").is_err()); // bad hour
    }
}
