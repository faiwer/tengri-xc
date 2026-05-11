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
    kmz,
};

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
    match format {
        InputFormat::Igc => {
            let raw = igc::decode_text(bytes);
            igc::parse_str(&raw).context("parsing IGC")
        }
        InputFormat::Kml => kml::parse_bytes(bytes).context("parsing KML"),
        InputFormat::Kmz => kmz::parse_bytes(bytes).context("parsing KMZ"),
        InputFormat::Gpx => gpx::parse_bytes(bytes).context("parsing GPX"),
    }
}

pub fn parse_input(input: &Path) -> anyhow::Result<Track> {
    let format = detect_format(input)?;
    let bytes = std::fs::read(input).with_context(|| format!("reading {}", input.display()))?;
    parse_format(format, &bytes)
}

/// Translate the upload as it lives on disk into the bytes we store
/// in `flight_sources` and the matching `flight_source_format` enum
/// value.
///
/// For IGC/KML/GPX this is identity. For KMZ we unzip and store the
/// inner KML — `flight_sources` then carries a value that
/// `upgrade-tracks` can re-parse without ever needing to know the
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
    let takeoff_at = track.points[window.takeoff_idx].time as i64;
    let landing_at = track.points[window.landing_idx].time as i64;

    let compact = encode(&track)
        .context("encoding compact track")
        .map_err(PrepareError::Encode)?;
    let envelope = TengriFile::new(Metadata::default(), compact);
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
