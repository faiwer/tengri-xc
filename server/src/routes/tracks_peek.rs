//! `POST /tracks/peek` — parse and score an uploaded flight without saving it.

use std::{io::Read, path::Path, time::Instant};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, multipart::MultipartRejection},
    routing::post,
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use flate2::read::GzDecoder;
use serde::Serialize;
use tengri_geo::{
    PointDegrees, project_track_points_m, rdp_indexes_with_chord_cap,
    simplify_track_for_scoring_with_chord_cap,
};
use tokio::task;

use crate::{
    AppError, AppState,
    auth::{Identity, require_permission},
    flight::{
        Metadata, Route, ScoringOutcome, TengriFile, Track, TrackPoint, encode, evaluate_routes,
        find_flight_window,
        ingest::{InputFormat, detect_format, parse_format, slice_flight_window},
        kmz, timezone,
    },
    user::Permissions,
};

/// Even huge KML flights are < 1 MIB in GZip.
const MAX_UPLOAD_BYTES: usize = 2 * 1024 * 1024;
/// GunZip bomb protection.
const MAX_DECOMPRESSED_FLIGHT_BYTES: usize = 32 * 1024 * 1024;
/// Too curious script kiddies might want to check our limits.
const MAX_TRACK_POINTS: usize = 300_000;
const SCORING_RDP_TOLERANCE_M: f64 = 500.0;
const SCORING_RDP_CHORD_CAP_M: f64 = 500.0;
/// No need to show the whole track in the preview.
const PREVIEW_RDP_TOLERANCE_M: f64 = 300.0;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/tracks/peek",
        post(peek).layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES)),
    )
}

async fn peek(
    identity: Identity,
    multipart: Result<Multipart, MultipartRejection>,
) -> Result<Json<PeekResponse>, AppError> {
    require_permission(&identity, Permissions::CAN_AUTHORIZE)?;

    let upload = parse_form_multipart(multipart?).await?;
    // Since it can be CPU-intensive, run it in a blocking task.
    let response = task::spawn_blocking(move || process_upload(upload))
        .await
        .map_err(|e| AppError::Internal(e.into()))??;

    Ok(Json(response))
}

fn process_upload(upload: Upload) -> Result<PeekResponse, AppError> {
    let ParsedUpload {
        track,
        source_format,
    } = parse_upload_track(&upload)?;
    let flight_metadata = build_flight_metadata(&track)?;

    let scoring = score(&track, flight_metadata.window)?;

    let preview_track = simplify_for_preview(&track);
    let flight_points = preview_track.points.len();
    let flight = encode_preview_flight(preview_track, flight_metadata.tengri.clone())?;

    Ok(PeekResponse {
        flight,
        metadata: PeekMetadata {
            takeoff_at: flight_metadata.takeoff.time as i64,
            landing_at: flight_metadata.landing.time as i64,
            takeoff_timezone: flight_metadata.takeoff_timezone,
            landing_timezone: flight_metadata.landing_timezone,
            takeoff: flight_metadata.takeoff_point,
            landing: flight_metadata.landing_point,
            source_format,
            source_points: track.points.len(),
            flight_points,
            scoring_points: scoring.points,
            scoring_ms: scoring.ms,
            routes: scoring.routes,
        },
    })
}

struct FlightMetadata {
    window: crate::flight::FlightWindow,
    takeoff: TrackPoint,
    landing: TrackPoint,
    takeoff_timezone: String,
    landing_timezone: String,
    takeoff_point: PointDegrees,
    landing_point: PointDegrees,
    tengri: Metadata,
}

fn build_flight_metadata(track: &Track) -> Result<FlightMetadata, AppError> {
    let window = find_flight_window(track)
        .ok_or_else(|| AppError::BadRequest("no takeoff/landing detected".to_owned()))?;
    let takeoff = track.points[window.takeoff_idx];
    let landing = track.points[window.landing_idx];
    let takeoff_timezone = timezone::name_at(takeoff.lat, takeoff.lon);
    let landing_timezone = timezone::name_at(landing.lat, landing.lon);
    let tengri = Metadata {
        takeoff_timezone: takeoff_timezone.clone(),
        landing_timezone: landing_timezone.clone(),
        takeoff_lat: takeoff.lat,
        takeoff_lon: takeoff.lon,
        landing_lat: landing.lat,
        landing_lon: landing.lon,
    };

    Ok(FlightMetadata {
        window,
        takeoff,
        landing,
        takeoff_timezone,
        landing_timezone,
        takeoff_point: PointDegrees::from_e5(takeoff.lat, takeoff.lon),
        landing_point: PointDegrees::from_e5(landing.lat, landing.lon),
        tengri,
    })
}

struct ScoredWindow {
    points: usize,
    ms: u32,
    routes: Vec<Route>,
}

fn score(track: &Track, window: crate::flight::FlightWindow) -> Result<ScoredWindow, AppError> {
    let scoring_track = simplify_for_scoring(&slice_flight_window(track.clone(), window));
    let scoring_started = Instant::now();
    let evaluation = match evaluate_routes(&scoring_track) {
        ScoringOutcome::Answer(evaluation) => evaluation,
        ScoringOutcome::NoAnswer => {
            return Err(AppError::BadRequest(
                "scoring produced no route evaluation".to_owned(),
            ));
        }
        ScoringOutcome::Error(error) => {
            return Err(AppError::BadRequest(format!("scoring failed: {error}")));
        }
    };
    let ms = scoring_started.elapsed().as_millis() as u32;
    let routes = evaluation
        .routes
        .into_iter()
        .filter_map(|outcome| match outcome {
            ScoringOutcome::Answer(route) => Some(route),
            ScoringOutcome::NoAnswer | ScoringOutcome::Error(_) => None,
        })
        .collect::<Vec<_>>();

    Ok(ScoredWindow {
        points: scoring_track.points.len(),
        ms,
        routes,
    })
}

fn simplify_for_scoring(track: &Track) -> Track {
    let indexes = simplify_track_for_scoring_with_chord_cap(
        &track.points,
        SCORING_RDP_TOLERANCE_M,
        SCORING_RDP_CHORD_CAP_M,
    );
    track.select_at(indexes)
}

struct ParsedUpload {
    track: Track,
    source_format: String,
}

fn parse_upload_track(upload: &Upload) -> Result<ParsedUpload, AppError> {
    let (format, source_format) = format_from_filename(&upload.filename)?;
    let raw = try_unzip(upload)?;
    let (format, raw) = normalize_upload(format, raw)?;
    let track = parse_format(format, &raw)
        .map_err(|e| AppError::BadRequest(format!("unsupported or invalid track file: {e:#}")))?;

    if track.points.len() > MAX_TRACK_POINTS {
        return Err(AppError::BadRequest(format!(
            "track has {} points; maximum is {MAX_TRACK_POINTS}",
            track.points.len()
        )));
    }

    Ok(ParsedUpload {
        track,
        source_format,
    })
}

struct Upload {
    filename: String,
    bytes: Vec<u8>,
}

async fn parse_form_multipart(mut multipart: Multipart) -> Result<Upload, AppError> {
    let mut upload: Option<Upload> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("invalid multipart body: {e}")))?
    {
        let name = field
            .name()
            .ok_or_else(|| AppError::BadRequest("multipart field is missing a name".to_owned()))?;
        if name != "flight" {
            return Err(AppError::BadRequest(format!(
                "unexpected multipart field `{name}`"
            )));
        }
        if upload.is_some() {
            return Err(AppError::BadRequest(
                "multipart body must contain exactly one `flight` field".to_owned(),
            ));
        }

        let filename = field
            .file_name()
            .ok_or_else(|| {
                AppError::BadRequest("`flight` field must include a filename".to_owned())
            })?
            .to_owned();
        let bytes = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("failed to read `flight` field: {e}")))?
            .to_vec();
        upload = Some(Upload { filename, bytes });
    }

    upload.ok_or_else(|| AppError::BadRequest("missing `flight` field".to_owned()))
}

impl From<MultipartRejection> for AppError {
    fn from(err: MultipartRejection) -> Self {
        AppError::BadRequest(format!("invalid multipart upload: {err}"))
    }
}

fn try_unzip(upload: &Upload) -> Result<Vec<u8>, AppError> {
    if has_gzip_magic(&upload.bytes) {
        return gunzip(&upload.bytes, MAX_DECOMPRESSED_FLIGHT_BYTES);
    }
    Ok(upload.bytes.clone())
}

fn gunzip(bytes: &[u8], limit: usize) -> Result<Vec<u8>, AppError> {
    let mut out = Vec::new();
    let mut decoder = GzDecoder::new(bytes);
    // Take "limit + 1" to prevent gunzip bomb attacks.
    let mut limited = decoder.by_ref().take((limit + 1) as u64);
    limited
        .read_to_end(&mut out)
        .map_err(|e| AppError::BadRequest(format!("failed to gunzip `flight`: {e}")))?;
    if out.len() > limit {
        return Err(AppError::BadRequest(format!(
            "decompressed `flight` exceeds the {limit} byte limit"
        )));
    }
    Ok(out)
}

/// KMZ is a ZIP-wrapper over KML. Unzip it when needed.
fn normalize_upload(format: InputFormat, raw: Vec<u8>) -> Result<(InputFormat, Vec<u8>), AppError> {
    match format {
        InputFormat::Kmz => {
            let inner = kmz::extract_kml_bytes_bounded(&raw, MAX_DECOMPRESSED_FLIGHT_BYTES)
                .map_err(|e| AppError::BadRequest(format!("invalid KMZ: {e}")))?;
            Ok((InputFormat::Kml, inner))
        }
        _ => Ok((format, raw)),
    }
}

fn format_from_filename(filename: &str) -> Result<(InputFormat, String), AppError> {
    let normalized = strip_gzip_suffix(filename);
    let format = detect_format(Path::new(normalized))
        .map_err(|e| AppError::BadRequest(format!("unsupported track format: {e:#}")))?;
    let source_format = format.pg_enum_value().to_owned();
    Ok((format, source_format))
}

fn strip_gzip_suffix(filename: &str) -> &str {
    if filename.to_ascii_lowercase().ends_with(".gz") {
        &filename[..filename.len() - 3]
    } else {
        filename
    }
}

/// Check the mime-type
fn has_gzip_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1f, 0x8b])
}

fn simplify_for_preview(track: &Track) -> Track {
    if track.points.len() <= 2 {
        return track.clone();
    }

    let indexes = rdp_indexes_with_chord_cap(
        &project_track_points_m(&track.points),
        PREVIEW_RDP_TOLERANCE_M,
        None,
    );
    track.select_at(indexes)
}

fn encode_preview_flight(track: Track, metadata: Metadata) -> Result<String, AppError> {
    let compact = encode(&track).map_err(|e| AppError::Internal(e.into()))?;
    let file = TengriFile::new(metadata, compact);
    let bytes = file
        .to_bincode_bytes()
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(B64.encode(bytes))
}

#[derive(Serialize)]
struct PeekResponse {
    flight: String,
    metadata: PeekMetadata,
}

#[derive(Serialize)]
struct PeekMetadata {
    takeoff_at: i64,
    landing_at: i64,
    takeoff_timezone: String,
    landing_timezone: String,
    takeoff: PointDegrees,
    landing: PointDegrees,
    source_format: String,
    source_points: usize,
    flight_points: usize,
    scoring_points: usize,
    scoring_ms: u32,
    routes: Vec<Route>,
}
