//! `POST /me/flights` — persist an uploaded flight (part 1) and enqueue route
//! scoring (part 2). The flight `id` comes back immediately; `position` is the
//! caller's place in the scoring queue when scoring didn't finish within the
//! wait window, or `null` when it did.

use std::time::Duration;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, State, multipart::MultipartRejection},
    routing::post,
};
use serde::Serialize;
use tengri_formats::{detect_format, tengri::VERSION};
use tokio::task;

use crate::{
    AppError, AppState,
    auth::{Identity, require_permission},
    flight::{
        ingest::{
            MAX_DECOMPRESSED_FLIGHT_BYTES, MAX_TRACK_POINTS, MAX_UPLOAD_BYTES, PrepareError,
            Prepared, gunzip_bounded, has_gzip_magic, prepare_bytes_for_storage,
        },
        store::{FlightRow, insert_flight, insert_source, insert_track, model_exists},
    },
    glider::{CATALOG_KINDS, LAUNCH_METHODS, PROPULSIONS},
    ids::nanoid_8,
    user::Permissions,
};

/// How long the request waits for scoring before returning a queue position.
const SCORING_WAIT: Duration = Duration::from_secs(3);

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/me/flights",
        post(create).layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES)),
    )
}

#[derive(Serialize)]
struct UploadResponse {
    /// The flight's unique identifier.
    id: String,
    /// `null` when scoring finished within the wait window; otherwise the
    /// number of jobs still ahead of this one in the queue.
    position: Option<i64>,
}

async fn create(
    State(state): State<AppState>,
    identity: Identity,
    multipart: Result<Multipart, MultipartRejection>,
) -> Result<Json<UploadResponse>, AppError> {
    require_permission(&identity, Permissions::CAN_AUTHORIZE)?;
    let user_id = identity.user_id;

    let form = parse_form(multipart?).await?;

    let exists = model_exists(
        state.pool(),
        user_id,
        &form.brand_id,
        &form.kind,
        &form.model_id,
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    if !exists {
        return Err(AppError::BadRequest(format!(
            "no glider {}/{}/{} available to you",
            form.brand_id, form.kind, form.model_id
        )));
    }

    let flight_id = persist_flight(&state, user_id, form).await?;

    // Subscribe before enqueuing so a fast worker can't finish before we listen.
    let rx = state.scoring_queue().register(&flight_id);
    state
        .scoring_queue()
        .enqueue(&flight_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    // Resolves the instant scoring finishes; the timeout is only the ceiling.
    let done = tokio::time::timeout(SCORING_WAIT, rx).await.is_ok();
    let position = if done {
        None
    } else {
        Some(
            state
                .scoring_queue()
                .position(&flight_id)
                .await
                .map_err(|e| AppError::Internal(e.into()))?,
        )
    };

    Ok(Json(UploadResponse {
        id: flight_id,
        position,
    }))
}

async fn persist_flight(
    state: &AppState,
    user_id: i32,
    form: UploadForm,
) -> Result<String, AppError> {
    let UploadForm {
        filename,
        bytes,
        kind,
        brand_id,
        model_id,
        launch_method,
        propulsion,
    } = form;

    // Parse + compact-encode + gzip is CPU-bound; keep it off the async runtime.
    let prepared = task::spawn_blocking(move || prepare_upload(&filename, bytes))
        .await
        .map_err(|e| AppError::Internal(e.into()))??;

    if prepared.track.points.len() > MAX_TRACK_POINTS {
        return Err(AppError::BadRequest(format!(
            "track has {} points; maximum is {MAX_TRACK_POINTS}",
            prepared.track.points.len()
        )));
    }

    let flight_id = nanoid_8();
    let mut tx = state
        .pool()
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    insert_flight(
        &mut tx,
        &FlightRow {
            flight_id: &flight_id,
            user_id,
            takeoff_at: prepared.takeoff_at,
            landing_at: prepared.landing_at,
            takeoff_timezone: &prepared.takeoff_timezone,
            landing_timezone: &prepared.landing_timezone,
            takeoff_lat: prepared.takeoff_lat,
            takeoff_lon: prepared.takeoff_lon,
            landing_lat: prepared.landing_lat,
            landing_lon: prepared.landing_lon,
            brand_id: &brand_id,
            kind: &kind,
            model_id: &model_id,
            propulsion: &propulsion,
            launch_method: &launch_method,
        },
    )
    .await
    .map_err(|err| AppError::Internal(err.into()))?;

    insert_source(
        &mut tx,
        &flight_id,
        prepared.format.pg_enum_value(),
        &prepared.source_gz,
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    insert_track(
        &mut tx,
        &flight_id,
        VERSION as i16,
        &prepared.etag,
        &prepared.track_bytes,
        prepared.compression_ratio,
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(flight_id)
}

/// Decompress (when gzipped) and turn the upload into storable bytes. KMZ is
/// unwrapped inside [`prepare_bytes_for_storage`].
fn prepare_upload(filename: &str, bytes: Vec<u8>) -> Result<Prepared, AppError> {
    let raw = if has_gzip_magic(&bytes) {
        gunzip_bounded(&bytes, MAX_DECOMPRESSED_FLIGHT_BYTES)
            .map_err(|e| AppError::BadRequest(format!("failed to gunzip flight: {e:#}")))?
    } else {
        bytes
    };
    let format = detect_format(std::path::Path::new(strip_gzip_suffix(filename)))
        .map_err(|e| AppError::BadRequest(format!("unsupported track format: {e:#}")))?;
    prepare_bytes_for_storage(format, raw).map_err(prepare_error)
}

fn strip_gzip_suffix(filename: &str) -> &str {
    if filename.to_ascii_lowercase().ends_with(".gz") {
        &filename[..filename.len() - 3]
    } else {
        filename
    }
}

fn prepare_error(err: PrepareError) -> AppError {
    match err {
        PrepareError::Parse(e) => {
            AppError::BadRequest(format!("unsupported or invalid track file: {e:#}"))
        }
        PrepareError::NoWindow => AppError::BadRequest("no takeoff/landing detected".to_owned()),
        PrepareError::Encode(e) | PrepareError::Io(e) => AppError::Internal(e),
    }
}

struct UploadForm {
    filename: String,
    bytes: Vec<u8>,
    kind: String,
    brand_id: String,
    model_id: String,
    launch_method: String,
    propulsion: String,
}

async fn parse_form(mut multipart: Multipart) -> Result<UploadForm, AppError> {
    let mut file: Option<(String, Vec<u8>)> = None;
    let mut kind: Option<String> = None;
    let mut brand_id: Option<String> = None;
    let mut model_id: Option<String> = None;
    let mut launch_method: Option<String> = None;
    let mut propulsion: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("invalid multipart body: {e}")))?
    {
        let name = field
            .name()
            .ok_or_else(|| AppError::BadRequest("multipart field is missing a name".to_owned()))?
            .to_owned();
        match name.as_str() {
            "flight" => {
                let filename = field
                    .file_name()
                    .ok_or_else(|| {
                        AppError::BadRequest("`flight` field must include a filename".to_owned())
                    })?
                    .to_owned();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| {
                        AppError::BadRequest(format!("failed to read `flight` field: {e}"))
                    })?
                    .to_vec();
                file = Some((filename, bytes));
            }
            "kind" => kind = Some(read_text(field, "kind").await?),
            "brand_id" => brand_id = Some(read_text(field, "brand_id").await?),
            "model_id" => model_id = Some(read_text(field, "model_id").await?),
            "launch_method" => launch_method = Some(read_text(field, "launch_method").await?),
            "propulsion" => propulsion = Some(read_text(field, "propulsion").await?),
            other => {
                return Err(AppError::BadRequest(format!(
                    "unexpected multipart field `{other}`"
                )));
            }
        }
    }

    let (filename, bytes) = file.ok_or_else(|| missing("flight"))?;
    let kind = require_one_of(kind.ok_or_else(|| missing("kind"))?, &CATALOG_KINDS, "kind")?;
    let launch_method = require_one_of(
        launch_method.ok_or_else(|| missing("launch_method"))?,
        &LAUNCH_METHODS,
        "launch_method",
    )?;
    let propulsion = require_one_of(
        propulsion.ok_or_else(|| missing("propulsion"))?,
        &PROPULSIONS,
        "propulsion",
    )?;

    Ok(UploadForm {
        filename,
        bytes,
        kind,
        brand_id: brand_id.ok_or_else(|| missing("brand_id"))?,
        model_id: model_id.ok_or_else(|| missing("model_id"))?,
        launch_method,
        propulsion,
    })
}

async fn read_text(
    field: axum::extract::multipart::Field<'_>,
    name: &str,
) -> Result<String, AppError> {
    field
        .text()
        .await
        .map_err(|e| AppError::BadRequest(format!("failed to read `{name}` field: {e}")))
}

fn require_one_of(value: String, allowed: &[&str], field: &str) -> Result<String, AppError> {
    if allowed.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "{field} must be one of: {}",
            allowed.join(", ")
        )))
    }
}

fn missing(field: &str) -> AppError {
    AppError::BadRequest(format!("missing `{field}` field"))
}
