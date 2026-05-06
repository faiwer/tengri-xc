//! Minimal IGC parser. Reads B-records and (optionally) the HFDTE date
//! header. Everything else (other H-records, L/I/J/E/F/K records, the
//! G-record signature) is silently skipped — that data lives in the
//! separate metadata table or is irrelevant to the geometry.
//!
//! # Coordinate / altitude conversion
//!
//! The IGC B-record packs latitude as `DDMMmmm` (degrees, minutes, and
//! thousandths of a minute) and altitudes as 5-digit integer meters.
//! The parser converts:
//!
//! - lat / lon  →  E5 micro-degrees (deg × 10⁵), via `deg + min/60` math.
//! - altitudes  →  decimeters (m × 10), preserving source resolution.
//!
//! # Time
//!
//! When `HFDTE` is present the parser produces real Unix epoch seconds.
//! When it's absent the parser falls back to seconds-since-midnight UTC,
//! which is still a monotonic `u32` suitable for the rest of the pipeline.
//! Midnight rollover is detected by a non-monotone HHMMSS and bumps an
//! internal day counter.

use crate::flight::types::{Track, TrackPoint};

use super::error::IgcError;

const B_MIN_LEN: usize = 35;

pub fn parse_str(input: &str) -> Result<Track, IgcError> {
    if input.is_empty() {
        return Err(IgcError::Empty);
    }

    let mut date_unix_days: Option<u32> = None;
    let mut time_acc = TimeAccumulator::default();
    let mut points: Vec<TrackPoint> = Vec::new();

    for (line_idx, line) in input.lines().enumerate() {
        let lineno = line_idx + 1;
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("HFDTE") {
            if let Some(days) = parse_hfdte(rest, lineno)? {
                date_unix_days = Some(days);
            }
            continue;
        }

        if line.starts_with('B') {
            let point = parse_b_record(line, lineno, &mut time_acc, date_unix_days)?;
            points.push(point);
            continue;
        }

        // Other record types (H other than HFDTE, I, J, L, E, F, K, G) are
        // silently skipped: their data lives in the metadata pipeline.
    }

    if points.is_empty() {
        return Err(IgcError::NoFixes);
    }
    demote_zero_pressure(&mut points);

    let start_time = points[0].time;
    Ok(Track { start_time, points })
}

/// Parse the date portion of an HFDTE record. Recorders use either the
/// legacy form `HFDTEDDMMYY` or the modern `HFDTEDATE:DDMMYY,FF`; we just
/// grab the first 6 contiguous ASCII digits.
fn parse_hfdte(rest: &str, lineno: usize) -> Result<Option<u32>, IgcError> {
    let Some(date_str) = first_digits_run(rest, 6) else {
        return Ok(None);
    };
    let dd = parse_u32(&date_str[0..2], lineno)?;
    let mm = parse_u32(&date_str[2..4], lineno)?;
    let yy = parse_u32(&date_str[4..6], lineno)?;
    // 2-digit year: 00..69 → 2000..2069, 70..99 → 1970..1999.
    let year = if yy < 70 { 2000 + yy } else { 1900 + yy };
    Ok(Some(date_to_unix_days(year, mm, dd, lineno)?))
}

fn parse_b_record(
    line: &str,
    lineno: usize,
    time_acc: &mut TimeAccumulator,
    date_unix_days: Option<u32>,
) -> Result<TrackPoint, IgcError> {
    if line.len() < B_MIN_LEN {
        return Err(IgcError::InvalidBRecord {
            line: lineno,
            reason: format!("expected at least {B_MIN_LEN} chars, got {}", line.len()),
        });
    }

    let total_seconds = parse_b_time(line, lineno, time_acc)?;
    let lat = parse_lat(line, lineno)?;
    let lon = parse_lon(line, lineno)?;
    // Validity A/V at byte 24 — accepted as-is. "V" is common before GPS lock.
    let pressure_alt_m = parse_alt5(&line[25..30], lineno)?;
    let geo_alt_m = parse_alt5(&line[30..35], lineno)?;

    let time = match date_unix_days {
        Some(days) => days * 86_400 + total_seconds,
        None => total_seconds,
    };

    Ok(TrackPoint {
        time,
        lat,
        lon,
        geo_alt: geo_alt_m * 10,
        pressure_alt: Some(pressure_alt_m * 10),
    })
}

fn parse_b_time(line: &str, lineno: usize, acc: &mut TimeAccumulator) -> Result<u32, IgcError> {
    let hh = parse_u32(&line[1..3], lineno)?;
    let mm = parse_u32(&line[3..5], lineno)?;
    let ss = parse_u32(&line[5..7], lineno)?;
    if hh > 23 || mm > 59 || ss > 59 {
        return Err(IgcError::InvalidBRecord {
            line: lineno,
            reason: format!("bad time HHMMSS={hh:02}{mm:02}{ss:02}"),
        });
    }
    Ok(acc.advance(hh, mm, ss))
}

/// Latitude `DDMMmmmN/S` at byte positions 7..15.
fn parse_lat(line: &str, lineno: usize) -> Result<i32, IgcError> {
    let deg = parse_u32(&line[7..9], lineno)?;
    let min_thousandths = parse_u32(&line[9..14], lineno)?;
    let abs = degmin_to_e5(deg, min_thousandths);
    match line.as_bytes()[14] {
        b'N' => Ok(abs),
        b'S' => Ok(-abs),
        other => Err(IgcError::InvalidBRecord {
            line: lineno,
            reason: format!("invalid lat hemisphere {:?}", other as char),
        }),
    }
}

/// Longitude `DDDMMmmmE/W` at byte positions 15..24.
fn parse_lon(line: &str, lineno: usize) -> Result<i32, IgcError> {
    let deg = parse_u32(&line[15..18], lineno)?;
    let min_thousandths = parse_u32(&line[18..23], lineno)?;
    let abs = degmin_to_e5(deg, min_thousandths);
    match line.as_bytes()[23] {
        b'E' => Ok(abs),
        b'W' => Ok(-abs),
        other => Err(IgcError::InvalidBRecord {
            line: lineno,
            reason: format!("invalid lon hemisphere {:?}", other as char),
        }),
    }
}

/// IGC convention: an all-zero pressure altitude column means the recorder
/// had no barometer. Demote those tracks to GPS-only.
fn demote_zero_pressure(points: &mut [TrackPoint]) {
    if points.iter().all(|p| p.pressure_alt == Some(0)) {
        for p in points.iter_mut() {
            p.pressure_alt = None;
        }
    }
}

/// Tracks midnight wraps inside a single flight. IGC B-records carry only
/// `HHMMSS`, so a non-monotone time means the clock has rolled over.
#[derive(Default)]
struct TimeAccumulator {
    day_offset: u32,
    prev: Option<u32>,
}

impl TimeAccumulator {
    fn advance(&mut self, hh: u32, mm: u32, ss: u32) -> u32 {
        let seconds_today = hh * 3600 + mm * 60 + ss;
        let mut total = self.day_offset * 86_400 + seconds_today;
        if let Some(prev) = self.prev
            && total < prev
        {
            self.day_offset += 1;
            total = self.day_offset * 86_400 + seconds_today;
        }
        self.prev = Some(total);
        total
    }
}

fn first_digits_run(s: &str, n: usize) -> Option<&str> {
    let bytes = s.as_bytes();
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b.is_ascii_digit() {
            if start.is_none() {
                start = Some(i);
            }
            if i + 1 - start.unwrap() == n {
                return Some(&s[start.unwrap()..=i]);
            }
        } else {
            start = None;
        }
    }
    None
}

fn parse_u32(s: &str, lineno: usize) -> Result<u32, IgcError> {
    s.parse::<u32>().map_err(|e| IgcError::InvalidBRecord {
        line: lineno,
        reason: format!("expected unsigned integer, got {s:?}: {e}"),
    })
}

fn parse_alt5(s: &str, lineno: usize) -> Result<i32, IgcError> {
    if s.len() != 5 {
        return Err(IgcError::InvalidBRecord {
            line: lineno,
            reason: format!("altitude field must be 5 chars, got {s:?}"),
        });
    }
    let parsed = if let Some(rest) = s.strip_prefix('-') {
        // Leading minus overlays the first digit: "-1234" = -1234 m.
        rest.parse::<i32>()
            .map(|v| -v)
            .map_err(|e| IgcError::InvalidBRecord {
                line: lineno,
                reason: format!("bad signed altitude {s:?}: {e}"),
            })?
    } else {
        s.parse::<i32>().map_err(|e| IgcError::InvalidBRecord {
            line: lineno,
            reason: format!("bad altitude {s:?}: {e}"),
        })?
    };
    Ok(parsed)
}

/// Convert IGC packed coordinates (degrees + thousandths-of-a-minute) to
/// E5 micro-degrees, rounded.
///
/// Source resolution is 1/60_000 deg ≈ 1.85 m. Output resolution is
/// 1/100_000 deg ≈ 1.11 m, which is finer than the source — no precision
/// is lost beyond the rounding of the final unit.
fn degmin_to_e5(deg: u32, min_thousandths: u32) -> i32 {
    // (deg + min_thousandths / 60_000) * 100_000
    //   = deg * 100_000 + min_thousandths * 100_000 / 60_000
    //   = deg * 100_000 + min_thousandths * 5 / 3      (with rounding)
    let whole = (deg as i64) * 100_000;
    let frac_num = (min_thousandths as i64) * 5;
    // Round half-up.
    let frac = (frac_num + 1) / 3;
    (whole + frac) as i32
}

/// Days from 1970-01-01 to the given (`year`, `month`, `day`).
/// Gregorian calendar; works for any year ≥ 1970.
fn date_to_unix_days(year: u32, month: u32, day: u32, lineno: usize) -> Result<u32, IgcError> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || year < 1970 {
        return Err(IgcError::InvalidBRecord {
            line: lineno,
            reason: format!("invalid HFDTE date {year:04}-{month:02}-{day:02}"),
        });
    }
    let days_in_month = [31u32, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = |y: u32| (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400);

    let mut days_before_year: u32 = 0;
    for y in 1970..year {
        days_before_year += if is_leap(y) { 366 } else { 365 };
    }

    let mut day_of_year: u32 = 0;
    for m in 1..month {
        let mut d = days_in_month[(m - 1) as usize];
        if m == 2 && is_leap(year) {
            d = 29;
        }
        day_of_year += d;
    }
    day_of_year += day - 1;

    Ok(days_before_year + day_of_year)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert!(matches!(parse_str(""), Err(IgcError::Empty)));
    }

    #[test]
    fn rejects_no_b_records() {
        let input = "HFDTEDATE:030526,01\n";
        assert!(matches!(parse_str(input), Err(IgcError::NoFixes)));
    }

    #[test]
    fn parses_minimal_track() {
        let input = "\
HFDTEDATE:030526,01
B1052024646349N01308989EA0166401735
B1052034646349N01308989EA0166301734
";
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 2);
        assert_eq!(t.points[0].pressure_alt, Some(16_640));
        assert_eq!(t.points[0].geo_alt, 17_350);
        // 2026-05-03 10:52:02 UTC = 1_777_805_522
        assert_eq!(t.start_time, 1_777_805_522);
        assert_eq!(t.points[1].time, 1_777_805_523);
    }

    #[test]
    fn handles_midnight_rollover() {
        let input = "\
HFDTEDATE:030526,01
B2359594600000N01300000EA0100001000
B0000004600000N01300000EA0100001000
";
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 2);
        assert_eq!(t.points[1].time, t.points[0].time + 1);
    }

    #[test]
    fn demotes_zero_pressure_to_gps_only() {
        let input = "\
HFDTEDATE:030526,01
B1052024646349N01308989EA0000001735
B1052034646349N01308989EA0000001734
";
        let t = parse_str(input).expect("parse");
        assert!(t.points.iter().all(|p| p.pressure_alt.is_none()));
    }

    #[test]
    fn degmin_conversion_is_correct() {
        // 46°46.349'  → 46 + 46.349/60 = 46.7724833…  →  E5 4677248
        assert_eq!(degmin_to_e5(46, 46_349), 4_677_248);
    }

    #[test]
    fn date_to_unix_days_is_correct() {
        // 2026-05-03  →  20_576 days since 1970-01-01.
        assert_eq!(date_to_unix_days(2026, 5, 3, 0).unwrap(), 20_576);
    }
}
