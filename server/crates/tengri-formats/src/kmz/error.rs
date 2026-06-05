use thiserror::Error;

use crate::KmlError;

#[derive(Debug, Error)]
pub enum KmzError {
    #[error("input is empty")]
    Empty,

    #[error("invalid ZIP archive: {0}")]
    InvalidZip(#[from] zip::result::ZipError),

    #[error("KMZ archive contains no KML entry (looked for `doc.kml`, then any `*.kml`)")]
    NoKmlEntry,

    #[error("failed to read KML entry from archive: {0}")]
    ReadEntry(#[source] std::io::Error),

    #[error("KML entry expands beyond the {limit} byte limit")]
    KmlEntryTooLarge { limit: usize },

    /// The inner KML failed to parse. Wrapping `KmlError` rather than
    /// flattening preserves the parser's structured error data
    /// (length mismatches, bad coords, etc.).
    #[error("inner KML failed to parse: {0}")]
    InnerKml(#[from] KmlError),
}
