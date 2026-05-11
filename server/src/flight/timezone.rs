//! Look up the UTC offset that applied at a given (lat, lon, UTC instant).
//!
//! Two layers stacked together:
//! 1. [`tzf_rs::DefaultFinder`] resolves a (lat, lon) to an IANA timezone name
//!    (e.g. `Asia/Almaty`) using bundled timezone polygons.
//! 2. [`chrono_tz`] turns that IANA name + a UTC instant into the offset that
//!    *historically* applied — so a flight in Kazakhstan on 2021-01-15 reads as
//!    `+21600` (UTC+6, pre-unification) while one on 2025-01-15 reads as
//!    `+18000` (UTC+5, post-unification). DST transitions and statutory rule
//!    changes are both handled by the embedded `tzdb`.
//!
//! The finder is allocated once and shared via [`OnceLock`]; constructing it
//! eagerly loads the embedded polygon blob, which is several MB.

use std::str::FromStr;
use std::sync::OnceLock;

use chrono::{DateTime, Offset, TimeZone, Utc};
use chrono_tz::Tz;
use tzf_rs::DefaultFinder;

static FINDER: OnceLock<DefaultFinder> = OnceLock::new();

fn finder() -> &'static DefaultFinder {
    FINDER.get_or_init(DefaultFinder::new)
}

/// UTC offset in whole seconds at the given fix. Positive = ahead of UTC.
///
/// Returns `0` (equivalent to `Etc/UTC`) when:
/// - tzf can't resolve the (lat, lon) — typically deep ocean, where the bundled
///   polygons run out.
/// - the resolved name isn't in `chrono_tz`'s embedded database, which
///   shouldn't happen with current versions but is handled defensively rather
///   than panicking.
///
/// `lat_e5` / `lon_e5` are the same E5 micro-degree integers
/// [`super::types::TrackPoint`] carries (`deg × 1e5`); `utc_seconds` is the
/// same Unix-epoch second `TrackPoint::time` carries widened to `i64`.
pub fn offset_seconds_at(lat_e5: i32, lon_e5: i32, utc_seconds: i64) -> i32 {
    let lat = lat_e5 as f64 / 1e5;
    let lon = lon_e5 as f64 / 1e5;

    let name = finder().get_tz_name(lon, lat);
    if name.is_empty() {
        return 0;
    }
    let Ok(tz) = Tz::from_str(name) else {
        return 0;
    };
    let Some(utc_dt) = DateTime::<Utc>::from_timestamp(utc_seconds, 0) else {
        return 0;
    };

    tz.offset_from_utc_datetime(&utc_dt.naive_utc())
        .fix()
        .local_minus_utc()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e5(deg: f64) -> i32 {
        (deg * 1e5).round() as i32
    }

    fn unix(rfc3339: &str) -> i64 {
        DateTime::parse_from_rfc3339(rfc3339).unwrap().timestamp()
    }

    /// Almaty before the 2024-03 unification: UTC+6.
    #[test]
    fn almaty_pre_2024_is_plus_six() {
        let off = offset_seconds_at(e5(43.25), e5(76.95), unix("2021-07-15T09:00:00Z"));
        assert_eq!(off, 6 * 3600);
    }

    /// Almaty after the 2024-03 unification: UTC+5.
    #[test]
    fn almaty_post_2024_is_plus_five() {
        let off = offset_seconds_at(e5(43.25), e5(76.95), unix("2025-01-15T09:00:00Z"));
        assert_eq!(off, 5 * 3600);
    }

    /// Vienna in July is on CEST (UTC+2).
    #[test]
    fn vienna_summer_is_cest() {
        let off = offset_seconds_at(e5(48.21), e5(16.37), unix("2025-07-15T12:00:00Z"));
        assert_eq!(off, 2 * 3600);
    }

    /// Vienna in January is on CET (UTC+1).
    #[test]
    fn vienna_winter_is_cet() {
        let off = offset_seconds_at(e5(48.21), e5(16.37), unix("2025-01-15T12:00:00Z"));
        assert_eq!(off, 3600);
    }

    /// Iceland abolished DST in 1968 and stays on UTC year-round.
    #[test]
    fn reykjavik_is_zero_year_round() {
        let summer = offset_seconds_at(e5(64.13), e5(-21.82), unix("2025-07-15T12:00:00Z"));
        let winter = offset_seconds_at(e5(64.13), e5(-21.82), unix("2025-01-15T12:00:00Z"));
        assert_eq!(summer, 0);
        assert_eq!(winter, 0);
    }

    /// Mid-Atlantic falls outside any land-zone polygon. tzf should return
    /// either an empty name or one of the `Etc/GMT*` ocean zones; either way
    /// the resolved offset for `(0, 0)` is 0.
    #[test]
    fn deep_ocean_falls_back_to_utc() {
        let off = offset_seconds_at(0, 0, unix("2025-07-15T12:00:00Z"));
        assert_eq!(off, 0);
    }
}
