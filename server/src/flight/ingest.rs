//! Server-side storage preparation for parsed flight formats.

use std::{
    io::{Read, Write},
    path::Path,
};

use anyhow::Context;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use tengri_formats::{
    FlightWindow, InputFormat, Metadata, TengriFile, Track, detect_format, encode,
    find_flight_window, normalize_for_storage, parse_format,
};

use crate::flight::{etag_for, timezone};

pub fn gzip_bytes(raw: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(raw)?;
    Ok(gz.finish()?)
}

pub fn gunzip_bytes(gz: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    GzDecoder::new(gz).read_to_end(&mut out)?;
    Ok(out)
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
    /// IANA timezone names at the takeoff/landing fixes. Mirrored into the
    /// `.tengri` [`Metadata`] and into `flights.*_timezone`.
    pub takeoff_timezone: String,
    pub landing_timezone: String,
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

    let takeoff_timezone = timezone::name_at(p_takeoff.lat, p_takeoff.lon);
    let landing_timezone = timezone::name_at(p_landing.lat, p_landing.lon);

    let metadata = Metadata {
        takeoff_timezone: takeoff_timezone.clone(),
        landing_timezone: landing_timezone.clone(),
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
        takeoff_timezone,
        landing_timezone,
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
