//! Cross-cutting helpers for taking a flight log on disk and turning
//! it into the bytes we want to store: format detection, dispatch to
//! the right parser, KMZ unwrapping, gzip. Lives in the library crate
//! (rather than in any one binary) so every ingest path — `tengri
//! add`, the leonardo importer, future HTTP uploads — uses the same
//! contract.

use std::path::Path;

use anyhow::{Context, anyhow};

use crate::{FlightWindow, Track, TrackPoint, find_flight_window, gpx, igc, kml, kmz};
use tengri_geo::approximate_distance_m;

const MAX_PLAUSIBLE_SPEED_MPS: f64 = 277.78; // 1000 km/h
const MAX_TRACK_GAP_SECONDS: u32 = 30 * 60; // 30 minutes
/// Drop points with more than 3 consecutive backward-time runs. If there are
/// more than 3 backward-time runs, we consider the track to be corrupted. Most
/// likely it has multiple tracks inside. In such a case we try to split
/// and keep the longest one.
const MAX_BACKWARD_TIME_POINTS_TO_DROP: usize = 3;
/// If we detected multiple backward time sequences, we split them into chunks.
/// But no more than `MAX_TIME_SEQUENCE_SEGMENTS` chunks.
const MAX_TIME_SEQUENCE_SEGMENTS: usize = 5;

/// Recognised input format. Wraps file-extension dispatch so the matching
/// `flight_source_format` enum value and the parser stay in lockstep. Add a
/// variant here whenever the parser zoo grows.
///
/// `Kmz` is a transport wrapper rather than a real parsed format —
/// [`normalize_for_storage`] cracks it open and downgrades it to `Kml` before
/// anything talks to the database, so the `flight_source_format` enum stays a
/// tidy `('igc', 'gpx', 'kml')`.
#[derive(Debug, Clone, Copy)]
pub enum InputFormat {
    Igc,
    Kml,
    Gpx,
    Kmz,
}

impl InputFormat {
    pub fn pg_enum_value(self) -> &'static str {
        match self {
            InputFormat::Igc => "igc",
            InputFormat::Kml | InputFormat::Kmz => "kml",
            InputFormat::Gpx => "gpx",
        }
    }

    pub fn from_pg_enum_value(value: &str) -> anyhow::Result<Self> {
        match value {
            "igc" => Ok(InputFormat::Igc),
            "kml" => Ok(InputFormat::Kml),
            "gpx" => Ok(InputFormat::Gpx),
            other => Err(anyhow!("unsupported source format `{other}`")),
        }
    }
}

pub fn detect_format(input: &Path) -> anyhow::Result<InputFormat> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("igc") => Ok(InputFormat::Igc),
        Some("kml") => Ok(InputFormat::Kml),
        Some("kmz") => Ok(InputFormat::Kmz),
        Some("gpx") => Ok(InputFormat::Gpx),
        Some(other) => Err(anyhow!("unsupported input format: .{other}")),
        None => Err(anyhow!(
            "input has no extension; cannot detect format: {}",
            input.display()
        )),
    }
}

pub fn parse_format(format: InputFormat, bytes: &[u8]) -> anyhow::Result<Track> {
    let track = match format {
        InputFormat::Igc => {
            let raw = igc::decode_text(bytes);
            igc::parse_str(&raw).context("parsing IGC")
        }
        InputFormat::Kml => kml::parse_bytes(bytes).context("parsing KML"),
        InputFormat::Kmz => kmz::parse_bytes(bytes).context("parsing KMZ"),
        InputFormat::Gpx => gpx::parse_bytes(bytes).context("parsing GPX"),
    }?;
    clean_track_points(track)
}

pub fn parse_input(input: &Path) -> anyhow::Result<Track> {
    let format = detect_format(input)?;
    let bytes = std::fs::read(input).with_context(|| format!("reading {}", input.display()))?;
    parse_format(format, &bytes)
}

pub fn slice_flight_window(track: Track, window: FlightWindow) -> Track {
    let points = track.points[window.takeoff_idx..=window.landing_idx].to_vec();
    Track {
        start_time: points[0].time,
        points,
    }
}

/// The same as `slice_flight_window`, but for a specific time range, instrad of index range.
pub fn slice_time_range(track: Track, takeoff_at: i64, landing_at: i64) -> anyhow::Result<Track> {
    let takeoff_at =
        u32::try_from(takeoff_at).context("stored takeoff timestamp does not fit in u32")?;
    let landing_at =
        u32::try_from(landing_at).context("stored landing timestamp does not fit in u32")?;
    let start = track
        .points
        .iter()
        .position(|point| point.time >= takeoff_at)
        .ok_or_else(|| anyhow!("source track has no point at or after stored takeoff"))?;
    let end = track
        .points
        .iter()
        .rposition(|point| point.time <= landing_at)
        .ok_or_else(|| anyhow!("source track has no point at or before stored landing"))?;
    if end < start {
        return Err(anyhow!(
            "stored takeoff/landing window does not overlap source track"
        ));
    }

    Ok(Track {
        start_time: track.points[start].time,
        points: track.points[start..=end].to_vec(),
    })
}

fn clean_track_points(track: Track) -> anyhow::Result<Track> {
    let track = drop_same_time_points(track);

    if has_time_shift_sequence(&track.points) {
        return clean_time_shift_sequences(&track)
            .ok_or_else(|| anyhow!("source track has unrecoverable backward timestamp sequences"));
    }

    Ok(finish_cleaning(drop_short_backward_time_runs(track)))
}

/// Collapse runs of same-timestamp fixes to the first one. Two fixes sharing a
/// second is a tracking-device bug — pick one and move on so every later stage
/// can rely on strictly increasing (or properly backward, which the next step
/// handles) timestamps.
fn drop_same_time_points(track: Track) -> Track {
    if track
        .points
        .windows(2)
        .all(|pair| pair[0].time != pair[1].time)
    {
        return track;
    }

    let mut points: Vec<TrackPoint> = Vec::with_capacity(track.points.len());
    for &point in &track.points {
        if points.last().is_some_and(|last| last.time == point.time) {
            continue;
        }
        points.push(point);
    }

    Track {
        start_time: points
            .first()
            .map(|point| point.time)
            .unwrap_or(track.start_time),
        points,
    }
}

/// Apply the shared post-time-cleaning filters.
fn finish_cleaning(track: Track) -> Track {
    select_longest_realistic_segment(drop_implausible_points(track))
}

/// Detect a sustained backward-time run that is too long to treat as noise.
fn has_time_shift_sequence(points: &[TrackPoint]) -> bool {
    let mut idx = 1;
    while idx < points.len() {
        if points[idx].time >= points[idx - 1].time {
            idx += 1;
            continue;
        }

        let previous_time = points[idx - 1].time;
        let start = idx;
        while idx < points.len() && points[idx].time <= previous_time {
            idx += 1;
        }
        if idx - start > MAX_BACKWARD_TIME_POINTS_TO_DROP {
            return true;
        }
    }

    false
}

/// Drop short backward-time runs as isolated bad fixes.
fn drop_short_backward_time_runs(track: Track) -> Track {
    if track
        .points
        .windows(2)
        .all(|pair| pair[1].time >= pair[0].time)
    {
        return track;
    }

    let mut points = Vec::with_capacity(track.points.len());
    let mut idx = 0;

    while idx < track.points.len() {
        let Some(previous) = points.last().copied() else {
            points.push(track.points[idx]);
            idx += 1;
            continue;
        };

        if track.points[idx].time >= previous.time {
            points.push(track.points[idx]);
            idx += 1;
            continue;
        }

        let previous_time = previous.time;
        while idx < track.points.len() && track.points[idx].time <= previous_time {
            idx += 1;
        }
    }

    Track {
        start_time: points
            .first()
            .map(|point| point.time)
            .unwrap_or(track.start_time),
        points,
    }
}

/// Split shifted timelines, join non-overlapping pieces, and keep the best flight.
fn clean_time_shift_sequences(track: &Track) -> Option<Track> {
    let segments = time_sequence_segments(&track.points);
    if segments.len() > MAX_TIME_SEQUENCE_SEGMENTS {
        return None;
    }

    let candidates = joined_time_sequence_candidates(&segments);
    let mut best: Option<(Track, u32)> = None;

    for candidate in candidates {
        let cleaned = finish_cleaning(candidate);
        let Some(duration) = flight_window_duration(&cleaned) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(_, best_duration)| duration > *best_duration)
        {
            best = Some((cleaned, duration));
        }
    }

    best.map(|(track, _)| track)
}

/// Split the source track into file-order chunks with monotone timestamps.
fn time_sequence_segments(points: &[TrackPoint]) -> Vec<Vec<TrackPoint>> {
    let mut segments = Vec::new();
    let mut start = 0;

    for idx in 1..points.len() {
        if points[idx].time < points[idx - 1].time {
            segments.push(points[start..idx].to_vec());
            start = idx;
        }
    }
    if start < points.len() {
        segments.push(points[start..].to_vec());
    }

    segments
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .collect()
}

/// Build candidate tracks by joining timestamp-ordered chunks that do not overlap.
fn joined_time_sequence_candidates(segments: &[Vec<TrackPoint>]) -> Vec<Track> {
    let mut candidates = Vec::new();
    let mut sorted_segments: Vec<&[TrackPoint]> = segments.iter().map(Vec::as_slice).collect();
    sorted_segments.sort_by_key(|segment| segment.first().map(|point| point.time));

    for start_idx in 0..sorted_segments.len() {
        let mut points = Vec::new();
        let mut last_time: Option<u32> = None;
        for segment in &sorted_segments[start_idx..] {
            let Some(first) = segment.first() else {
                continue;
            };
            if last_time.is_none_or(|time| first.time > time) {
                points.extend_from_slice(segment);
                last_time = points.last().map(|point| point.time);
            }
        }
        if let Some(first) = points.first() {
            candidates.push(Track {
                start_time: first.time,
                points,
            });
        }
    }

    candidates
}

/// Duration of the detected flight window, used to rank cleanup candidates.
fn flight_window_duration(track: &Track) -> Option<u32> {
    let window = find_flight_window(track)?;
    Some(
        track.points[window.landing_idx]
            .time
            .saturating_sub(track.points[window.takeoff_idx].time),
    )
}

/// Drop white-crow points (duplicate timestamps, GPS jumps above 1000 km/h).
/// Interior outliers are easy — compare each point against the last *kept* one
/// and drop the ones that don't reach. The hard case is when the very first fix
/// is the outlier, see [`trusted_head_idx`].
fn drop_implausible_points(track: Track) -> Track {
    let head = trusted_head_idx(&track.points);
    let Some(&first) = track.points.get(head) else {
        return track;
    };

    let mut kept = Vec::with_capacity(track.points.len() - head);
    kept.push(first);
    for &point in &track.points[head + 1..] {
        if plausible_step(*kept.last().unwrap(), point) {
            kept.push(point);
        }
    }

    Track {
        start_time: kept[0].time,
        points: kept,
    }
}

/// Index of the first point we trust as the anchor for filtering.
///
/// GPS cold-start can emit a garbage first fix (wrong city) before the receiver
/// locks. The interior filter in [`drop_implausible_points`] anchors on the
/// previous *kept* point, so anchoring on a bad `points[0]` would reject every
/// real fix as unreachable.
///
/// If `0 → 1` is implausible *and* `1 → 2` is plausible, the rest of the track
/// is self-consistent without `0` — so `0` is the crow and we skip past it.
/// Otherwise we anchor on `0` and let the interior loop sort it out.
fn trusted_head_idx(points: &[TrackPoint]) -> usize {
    if points.len() >= 3
        && !plausible_step(points[0], points[1])
        && plausible_step(points[1], points[2])
    {
        1
    } else {
        0
    }
}

fn select_longest_realistic_segment(track: Track) -> Track {
    let Some((best_start, best_end)) = longest_realistic_segment_bounds(&track) else {
        return track;
    };

    if best_start == 0 && best_end == track.points.len() {
        return track;
    }

    // There were multiple chunks. Return the longest one as a new Track.
    let points = track.points[best_start..best_end].to_vec();
    let start_time = points[0].time;
    Track { start_time, points }
}

/// Split on >30 min gaps, keep only chunks with a detected flight window,
/// and return the `points[start..end]` bounds for the longest such window.
fn longest_realistic_segment_bounds(track: &Track) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize, u32)> = None;
    let mut start = 0;

    for idx in 1..=track.points.len() {
        let should_split = idx == track.points.len()
            || track.points[idx]
                .time
                .saturating_sub(track.points[idx - 1].time)
                > MAX_TRACK_GAP_SECONDS;
        if !should_split {
            continue;
        }

        if let Some(duration) = realistic_segment_duration(track, start, idx)
            && best
                .as_ref()
                .is_none_or(|&(_, _, best_duration)| duration > best_duration)
        {
            best = Some((start, idx, duration));
        }
        start = idx;
    }

    best.map(|(start, end, _)| (start, end))
}

fn realistic_segment_duration(track: &Track, start: usize, end: usize) -> Option<u32> {
    if end <= start + 1 {
        return None;
    }

    let points = track.points[start..end].to_vec();
    let segment = Track {
        start_time: points[0].time,
        points,
    };
    let window = find_flight_window(&segment)?;
    Some(
        segment.points[window.landing_idx]
            .time
            .saturating_sub(segment.points[window.takeoff_idx].time),
    )
}

/// Reject same-timestamp points and GPS jumps above 1000 km/h.
fn plausible_step(from: TrackPoint, to: TrackPoint) -> bool {
    let dt = to.time.saturating_sub(from.time);
    if dt == 0 {
        return false;
    }

    let distance_m = approximate_distance_m(from.lat, from.lon, to.lat, to.lon);
    distance_m / f64::from(dt) < MAX_PLAUSIBLE_SPEED_MPS
}

/// Translate the upload as it lives on disk into the bytes we store
/// in `flight_sources` and the matching `flight_source_format` enum
/// value.
///
/// For IGC/KML/GPX this is identity. For KMZ we unzip and store the
/// inner KML — `flight_sources` then carries a value that
/// `flight::backfill` can re-parse without ever needing to know the
/// upload was zipped, and the `flight_source_format` enum stays small.
pub fn normalize_for_storage(
    format: InputFormat,
    bytes: Vec<u8>,
) -> anyhow::Result<(InputFormat, Vec<u8>)> {
    match format {
        InputFormat::Kmz => {
            let inner = kmz::extract_kml_bytes(&bytes).context("extracting KML from KMZ")?;
            Ok((InputFormat::Kml, inner))
        }
        _ => Ok((format, bytes)),
    }
}

#[cfg(test)]
mod tests {
    use crate::{Track, TrackPoint, find_flight_window};

    use super::{InputFormat, clean_track_points, parse_format};

    fn point(time: u32, lat: i32, lon: i32) -> TrackPoint {
        TrackPoint {
            time,
            lat,
            lon,
            geo_alt: 0,
            pressure_alt: None,
            tas: None,
        }
    }

    fn flying_leg(t0: u32, n: usize, lon0: i32) -> Vec<TrackPoint> {
        (0..n)
            .map(|idx| point(t0 + idx as u32, 4_700_000, lon0 + idx as i32 * 20))
            .collect()
    }

    fn stationary(t0: u32, n: usize, lat: i32, lon: i32) -> Vec<TrackPoint> {
        (0..n).map(|idx| point(t0 + idx as u32, lat, lon)).collect()
    }

    fn midnight_date_change_igc() -> String {
        let mut out = String::from("HFDTE030526\n");
        for idx in 0..600 {
            let seconds = (23 * 3_600 + 55 * 60 + idx) % 86_400;
            if idx == 300 {
                out.push_str("HFDTE040526\n");
            }
            let hh = seconds / 3_600;
            let mm = (seconds % 3_600) / 60;
            let ss = seconds % 60;
            let lon_minutes = idx * 10;
            out.push_str(&format!(
                "B{hh:02}{mm:02}{ss:02}4700000N008{lon_minutes:05}EA0100001000\n"
            ));
        }
        out
    }

    #[test]
    fn drops_impossible_cluster_until_track_returns_to_normal() {
        let track = Track {
            start_time: 0,
            points: vec![
                point(0, 0, 0),
                point(60, 0, 1_000),
                point(120, 0, 2_000),
                point(180, 0, 999_999),
                point(240, 0, 999_998),
                point(300, 0, 4_000),
            ],
        };

        let cleaned = clean_track_points(track).unwrap();

        assert_eq!(
            cleaned
                .points
                .iter()
                .map(|point| point.lon)
                .collect::<Vec<_>>(),
            vec![0, 1_000, 2_000, 4_000]
        );
    }

    #[test]
    fn drops_implausible_leading_point_when_next_step_is_sane() {
        let track = Track {
            start_time: 0,
            points: vec![
                point(0, 4_323_043, 7_510_220),
                point(2, 4_307_675, 7_627_848),
                point(2, 4_307_675, 7_627_848),
                point(4, 4_307_670, 7_627_859),
                point(6, 4_307_658, 7_627_874),
            ],
        };

        let cleaned = clean_track_points(track).unwrap();

        assert_eq!(
            cleaned.points,
            vec![
                point(2, 4_307_675, 7_627_848),
                point(4, 4_307_670, 7_627_859),
                point(6, 4_307_658, 7_627_874),
            ]
        );
        assert_eq!(cleaned.start_time, 2);
    }

    #[test]
    fn drops_implausible_leading_point_after_collapsing_same_time_fix() {
        let track = Track {
            start_time: 0,
            points: vec![
                point(0, 4_323_043, 7_510_220),
                point(2, 4_307_675, 7_627_848),
                point(2, 4_307_675, 7_627_848),
                point(4, 4_307_675, 7_627_848),
                point(6, 4_307_670, 7_627_859),
            ],
        };

        let cleaned = clean_track_points(track).unwrap();

        assert_eq!(
            cleaned.points,
            vec![
                point(2, 4_307_675, 7_627_848),
                point(4, 4_307_675, 7_627_848),
                point(6, 4_307_670, 7_627_859),
            ]
        );
        assert_eq!(cleaned.start_time, 2);
    }

    #[test]
    fn drops_exact_duplicate_points() {
        let duplicate = point(60, 0, 1_000);
        let track = Track {
            start_time: 0,
            points: vec![point(0, 0, 0), duplicate, duplicate, point(120, 0, 2_000)],
        };

        let cleaned = clean_track_points(track).unwrap();

        assert_eq!(
            cleaned.points,
            vec![point(0, 0, 0), duplicate, point(120, 0, 2_000)]
        );
    }

    #[test]
    fn drops_same_timestamp_points() {
        let mut same_time = point(60, 0, 1_000);
        same_time.geo_alt = 10;
        let track = Track {
            start_time: 0,
            points: vec![
                point(0, 0, 0),
                point(60, 0, 1_000),
                same_time,
                point(120, 0, 2_000),
            ],
        };

        let cleaned = clean_track_points(track).unwrap();

        assert_eq!(
            cleaned.points,
            vec![point(0, 0, 0), point(60, 0, 1_000), point(120, 0, 2_000)]
        );
    }

    #[test]
    fn drops_short_backward_time_run_as_white_crow_points() {
        let track = Track {
            start_time: 0,
            points: vec![
                point(0, 0, 0),
                point(10, 0, 100),
                point(9, 0, 110),
                point(8, 0, 120),
                point(20, 0, 200),
                point(30, 0, 300),
            ],
        };

        let cleaned = clean_track_points(track).unwrap();

        assert_eq!(
            cleaned.points,
            vec![
                point(0, 0, 0),
                point(10, 0, 100),
                point(20, 0, 200),
                point(30, 0, 300),
            ]
        );
    }

    #[test]
    fn changing_date_header_at_midnight_does_not_split_track() {
        let input = midnight_date_change_igc();
        let track = parse_format(InputFormat::Igc, input.as_bytes()).unwrap();
        let window = find_flight_window(&track).expect("should detect flight");

        assert_eq!(track.points.len(), 600);
        assert_eq!(track.points[300].time, track.points[299].time + 1);
        assert!(window.takeoff_idx < 10);
        assert_eq!(window.landing_idx, track.points.len() - 1);
    }

    #[test]
    fn splits_long_backward_time_run_and_keeps_best_flight_candidate() {
        let mut points = flying_leg(0, 300, 800_000);
        points.extend(stationary(3_000, 10, 4_700_000, 900_000));
        points.extend(flying_leg(100, 600, 1_000_000));
        points.extend(stationary(3_100, 10, 4_700_000, 1_100_000));
        let track = Track {
            start_time: 0,
            points,
        };

        let cleaned = clean_track_points(track).unwrap();

        assert_eq!(cleaned.start_time, 100);
        assert_eq!(cleaned.points.len(), 600);
    }

    #[test]
    fn joins_time_sequence_candidates_in_timestamp_order() {
        let early = vec![point(100, 0, 1), point(110, 0, 2)];
        let middle = vec![point(200, 0, 3), point(210, 0, 4)];
        let late = vec![point(300, 0, 5), point(310, 0, 6)];

        let candidates =
            super::joined_time_sequence_candidates(&[late.clone(), early.clone(), middle.clone()]);

        assert_eq!(candidates[0].points, [early, middle, late].concat());
    }

    #[test]
    fn bails_out_of_sequence_strategy_when_track_has_too_many_time_chunks() {
        let mut points = vec![point(1_000, 0, 0), point(1_001, 0, 1)];
        for idx in 0..super::MAX_TIME_SEQUENCE_SEGMENTS {
            let start = idx as u32 * 2;
            points.extend([
                point(start, 0, idx as i32),
                point(start + 1, 0, idx as i32 + 1),
            ]);
        }
        let track = Track {
            start_time: 100,
            points,
        };

        assert!(super::clean_time_shift_sequences(&track).is_none());
        assert!(clean_track_points(track).is_err());
    }

    #[test]
    fn time_shift_sequence_winner_has_monotone_time() {
        let mut points = flying_leg(0, 300, 800_000);
        points.extend(flying_leg(100, 600, 1_000_000));
        let track = Track {
            start_time: 0,
            points,
        };

        let cleaned = super::clean_time_shift_sequences(&track).unwrap();

        assert!(
            cleaned
                .points
                .windows(2)
                .all(|pair| pair[1].time > pair[0].time)
        );
    }

    #[test]
    fn ignores_stationary_segment_before_flight() {
        let mut points = stationary(0, 600, 4_700_000, 800_000);
        points.extend(flying_leg(3_000, 600, 800_000));
        let track = Track {
            start_time: 0,
            points,
        };

        let cleaned = clean_track_points(track).unwrap();

        assert_eq!(cleaned.start_time, 3_000);
        assert_eq!(cleaned.points.len(), 600);
    }

    #[test]
    fn chooses_longest_realistic_segment() {
        let mut points = flying_leg(0, 300, 800_000);
        points.extend(flying_leg(3_000, 600, 900_000));
        let track = Track {
            start_time: 0,
            points,
        };

        let cleaned = clean_track_points(track).unwrap();

        assert_eq!(cleaned.start_time, 3_000);
        assert_eq!(cleaned.points.len(), 600);
    }
}
