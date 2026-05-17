//! Cross-cutting helpers for taking a flight log on disk and turning
//! it into the bytes we want to store: format detection, dispatch to
//! the right parser, KMZ unwrapping, gzip. Lives in the library crate
//! (rather than in any one binary) so every ingest path — `tengri
//! add`, the leonardo importer, future HTTP uploads — uses the same
//! contract.

use std::{io::Write, path::Path};

use anyhow::{Context, anyhow};
use flate2::{Compression, write::GzEncoder};

use crate::flight::{
    FlightWindow, Metadata, TengriFile, Track, encode, etag_for, find_flight_window, gpx, igc, kml,
    kmz, timezone,
};
use crate::geo::approximate_distance_m;

const MAX_PLAUSIBLE_SPEED_MPS: f64 = 277.78; // 1000 km/h
const MAX_TRACK_GAP_SECONDS: u32 = 30 * 60; // 30 minutes

/// Recognised input format. Wraps file-extension dispatch so the
/// matching `flight_source_format` enum value and the parser stay in
/// lockstep. Add a variant here whenever the parser zoo grows.
///
/// `Kmz` is a transport wrapper rather than a real parsed format —
/// [`normalize_for_storage`] cracks it open and downgrades it to
/// `Kml` before anything talks to the database, so the
/// `flight_source_format` enum stays a tidy `('igc', 'gpx', 'kml')`.
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
    Ok(clean_track_points(track))
}

pub fn parse_input(input: &Path) -> anyhow::Result<Track> {
    let format = detect_format(input)?;
    let bytes = std::fs::read(input).with_context(|| format!("reading {}", input.display()))?;
    parse_format(format, &bytes)
}

fn clean_track_points(track: Track) -> Track {
    select_longest_realistic_segment(drop_implausible_points(track))
}

fn drop_implausible_points(track: Track) -> Track {
    let mut previous = match track.points.first().copied() {
        Some(point) => point,
        None => return track,
    };
    for (idx, point) in track.points.iter().copied().enumerate().skip(1) {
        if plausible_step(previous, point) {
            previous = point;
            continue;
        }
        return clean_track_points_from(track, idx);
    }

    track
}

/// Return a track without duplicate timestamps or implausible-speed points.
fn clean_track_points_from(track: Track, first_bad_idx: usize) -> Track {
    let mut points = Vec::with_capacity(track.points.len() - 1);
    points.extend_from_slice(&track.points[..first_bad_idx]);
    for point in track.points[first_bad_idx..].iter().copied() {
        let previous = points
            .last()
            .copied()
            .expect("first filtered index is never the first point");
        if plausible_step(previous, point) {
            points.push(point);
        }
    }

    Track {
        start_time: track.start_time,
        points,
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
fn plausible_step(
    from: crate::flight::types::TrackPoint,
    to: crate::flight::types::TrackPoint,
) -> bool {
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

pub fn gzip_bytes(raw: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(raw)?;
    Ok(gz.finish()?)
}

/// Everything an ingest path needs to write the three-row flight
/// bundle, plus the bits that callers like to log on success
/// (`window`, `track`). Built once by [`prepare_bytes_for_storage`]
/// (or its path wrapper [`prepare_path_for_storage`]) outside any
/// transaction; the `flight::store::insert_*` writers consume the
/// byte-blob half of it.
///
/// The `track` is kept in the result so callers that want to print
/// `n_points` or other per-track stats don't have to re-parse.
/// Callers that don't care can just ignore the field; they don't pay
/// any allocation cost beyond what parsing already does.
pub struct Prepared {
    pub format: InputFormat,
    pub track: Track,
    pub window: FlightWindow,
    pub takeoff_at: i64,
    pub landing_at: i64,
    /// UTC offset in whole seconds at the takeoff fix. Mirrored into
    /// the `.tengri` [`Metadata`] and into `flights.takeoff_offset`.
    pub takeoff_offset: i32,
    pub landing_offset: i32,
    /// E5 micro-degrees (deg × 10⁵), pulled straight off the takeoff
    /// fix. Bound into `flights.takeoff_point` after a degree conversion
    /// at the SQL boundary.
    pub takeoff_lat: i32,
    pub takeoff_lon: i32,
    pub landing_lat: i32,
    pub landing_lon: i32,
    /// `gzip(raw_upload_bytes)`. Goes into `flight_sources.bytes`.
    pub source_gz: Vec<u8>,
    /// `gzip(bincode(TengriFile))`. Goes into `flight_tracks.bytes` —
    /// the route handler streams this column verbatim.
    pub track_bytes: Vec<u8>,
    pub etag: String,
    /// `track_bytes.len() / source_gz.len()`. Stored in
    /// `flight_tracks.compression_ratio` so we can monitor the
    /// effectiveness of the compact encoder over time.
    pub compression_ratio: f32,
}

/// What can go wrong turning a flight log into [`Prepared`] bytes.
/// Modelled as variants (rather than one `anyhow::Error`) so binaries
/// that report per-row outcomes — the leonardo importer being the
/// motivating case — can categorise failures without parsing message
/// strings. Callers that don't care can `?` it through `anyhow` like
/// any other error.
///
/// The split mirrors how operators think about a bad row:
/// - `Parse`: the file is the wrong shape (bad extension, broken
///   IGC record, malformed XML). Re-running won't help; the source
///   needs replacing.
/// - `NoWindow`: the file parsed but there's no flying segment to
///   ingest. Usually a stationary log, occasionally a clipped track.
/// - `Encode`: the compact encoder or `TengriFile` envelope refused.
///   Should be impossible with parsed data; if it fires it's a bug.
/// - `Io`: filesystem (read, gzip). Mostly missing files.
#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    #[error("parse: {0:#}")]
    Parse(anyhow::Error),
    #[error("no takeoff/landing detected")]
    NoWindow,
    #[error("encode: {0:#}")]
    Encode(anyhow::Error),
    #[error("io: {0:#}")]
    Io(anyhow::Error),
}

/// Pre-compute every byte-blob the database will swallow, given the
/// upload bytes and the format we already decided they're in. Pure
/// CPU, no I/O — the caller has already done the read (filesystem,
/// HTTP body, mmap, whatever).
///
/// `raw` is consumed because `flight_sources` stores a gzipped copy
/// of exactly these bytes (after KMZ unwrap); taking `Vec<u8>` lets
/// us avoid a copy on the path that doesn't need one.
pub fn prepare_bytes_for_storage(
    format: InputFormat,
    raw: Vec<u8>,
) -> Result<Prepared, PrepareError> {
    let (format, raw) = normalize_for_storage(format, raw).map_err(PrepareError::Parse)?;
    let track = parse_format(format, &raw).map_err(PrepareError::Parse)?;

    let window = find_flight_window(&track).ok_or(PrepareError::NoWindow)?;
    let p_takeoff = track.points[window.takeoff_idx];
    let p_landing = track.points[window.landing_idx];
    let takeoff_at = p_takeoff.time as i64;
    let landing_at = p_landing.time as i64;

    let takeoff_offset = timezone::offset_seconds_at(p_takeoff.lat, p_takeoff.lon, takeoff_at);
    let landing_offset = timezone::offset_seconds_at(p_landing.lat, p_landing.lon, landing_at);

    let metadata = Metadata {
        takeoff_offset,
        landing_offset,
        takeoff_lat: p_takeoff.lat,
        takeoff_lon: p_takeoff.lon,
        landing_lat: p_landing.lat,
        landing_lon: p_landing.lon,
    };

    let compact = encode(&track)
        .context("encoding compact track")
        .map_err(PrepareError::Encode)?;
    let envelope = TengriFile::new(metadata, compact);
    let track_bytes = envelope
        .to_http_bytes()
        .context("encoding TengriFile to http bytes")
        .map_err(PrepareError::Encode)?;
    let etag = etag_for(&track_bytes);

    let source_gz = gzip_bytes(&raw)
        .context("gzipping source bytes")
        .map_err(PrepareError::Io)?;
    let compression_ratio = track_bytes.len() as f32 / source_gz.len() as f32;

    Ok(Prepared {
        format,
        track,
        window,
        takeoff_at,
        landing_at,
        takeoff_offset,
        landing_offset,
        takeoff_lat: p_takeoff.lat,
        takeoff_lon: p_takeoff.lon,
        landing_lat: p_landing.lat,
        landing_lon: p_landing.lon,
        source_gz,
        track_bytes,
        etag,
        compression_ratio,
    })
}

/// Path-based convenience wrapper around [`prepare_bytes_for_storage`].
/// Reads the file, sniffs the format from its extension, hands off to
/// the byte-based pipeline. This is what every CLI caller wants.
pub fn prepare_path_for_storage(input: &Path) -> Result<Prepared, PrepareError> {
    let format = detect_format(input).map_err(PrepareError::Parse)?;
    let raw = std::fs::read(input)
        .with_context(|| format!("reading {}", input.display()))
        .map_err(PrepareError::Io)?;
    prepare_bytes_for_storage(format, raw)
}

#[cfg(test)]
mod tests {
    use crate::flight::types::{Track, TrackPoint};

    use super::clean_track_points;

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

        let cleaned = clean_track_points(track);

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
    fn drops_exact_duplicate_points() {
        let duplicate = point(60, 0, 1_000);
        let track = Track {
            start_time: 0,
            points: vec![point(0, 0, 0), duplicate, duplicate, point(120, 0, 2_000)],
        };

        let cleaned = clean_track_points(track);

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

        let cleaned = clean_track_points(track);

        assert_eq!(
            cleaned.points,
            vec![point(0, 0, 0), point(60, 0, 1_000), point(120, 0, 2_000)]
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

        let cleaned = clean_track_points(track);

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

        let cleaned = clean_track_points(track);

        assert_eq!(cleaned.start_time, 3_000);
        assert_eq!(cleaned.points.len(), 600);
    }
}
