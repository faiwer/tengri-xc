use thiserror::Error;

#[derive(Debug, Error)]
pub enum KmlError {
    #[error("input is empty")]
    Empty,

    #[error("invalid XML: {0}")]
    InvalidXml(#[from] roxmltree::Error),

    #[error("input is not valid UTF-8: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    /// File parsed as XML but no recognised track shape was found.
    /// We accept GpsDumpAndroid track Placemarks, standard `<gx:Track>`
    /// documents, and GPSBabel/OGR `<Folder name="track_points">`
    /// fix streams; see the `parser` module for details.
    #[error(
        "no recognised track found in KML: expected GpsDumpAndroid track Placemark, <gx:Track>, or `track_points` Folder"
    )]
    NoTrack,

    #[error("track has no fixes")]
    NoFixes,

    #[error("track has timestamps and coordinates of different lengths: {times} vs {coords}")]
    LengthMismatch { times: usize, coords: usize },

    #[error("malformed coordinate triplet at index {index}: {reason}")]
    BadCoord { index: usize, reason: String },

    #[error("malformed timestamp at index {index}: {reason}")]
    BadTime { index: usize, reason: String },

    #[error("missing required element/attribute: {0}")]
    MissingElement(&'static str),
}
