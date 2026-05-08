//! Cross-subcommand utilities for the `tengri` CLI: format detection,
//! gzip helpers, NanoID generation, and Postgres connection.

use std::{io::Write, path::Path};

use anyhow::{Context, anyhow};
use flate2::{Compression, write::GzEncoder};
use rand::Rng;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tengri_server::flight::{Track, gpx, igc, kml, kmz};

/// Recognised input format. Wraps file-extension dispatch so the
/// matching `flight_source_format` enum value and the parser stay in
/// lockstep. Add a variant here whenever the parser zoo grows.
///
/// `Kmz` is a transport wrapper rather than a real parsed format —
/// `normalize_for_storage` cracks it open and downgrades it to `Kml`
/// before anything talks to the database, so the `flight_source_format`
/// enum stays a tidy `('igc', 'gpx', 'kml')`.
#[derive(Debug, Clone, Copy)]
pub enum InputFormat {
    Igc,
    Kml,
    Gpx,
    Kmz,
}

impl InputFormat {
    pub fn db_name(self) -> &'static str {
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

/// Translate the upload as it lives on disk into the bytes we store in
/// `flight_sources` and the matching `flight_source_format` enum value.
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

/// 8-char NanoID with the `[A-Za-z0-9_-]` alphabet declared in the schema
/// comment. 64 symbols × 8 chars = 48 bits of entropy, ample for the
/// expected row count and matching the spec exactly.
pub fn nanoid_8() -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Load `server/.env` and read `DATABASE_URL`. Used by every subcommand
/// that talks to Postgres directly *or* shells out to a tool that needs
/// the same connection string (e.g. `tengri db` → `psql`).
pub fn database_url() -> anyhow::Result<String> {
    let _ = dotenvy::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));
    std::env::var("DATABASE_URL").context("DATABASE_URL must be set (try server/.env)")
}

pub async fn connect_pool() -> anyhow::Result<PgPool> {
    let database_url = database_url()?;
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .context("connecting to Postgres")
}

pub async fn ensure_user_exists(pool: &PgPool, user_id: i32) -> anyhow::Result<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .context("checking user exists")?;
    if !exists {
        return Err(anyhow!("no user with id={user_id}"));
    }
    Ok(())
}

/// Spin up a single-threaded Tokio runtime on demand. The CLI is otherwise
/// fully synchronous; we don't want every subcommand paying for a runtime.
pub fn run_async<F>(fut: F) -> anyhow::Result<()>
where
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?
        .block_on(fut)
}
