use thiserror::Error;

#[derive(Debug, Error)]
pub enum GpxError {
    #[error("input is empty")]
    Empty,

    #[error("invalid XML: {0}")]
    InvalidXml(#[from] roxmltree::Error),

    #[error("input is not valid UTF-8: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    /// File parsed as XML but no `<trkpt>` elements were found.
    /// We don't accept route (`<rtept>`) or waypoint (`<wpt>`) data
    /// for v1: those are planning artifacts, not flown tracks.
    #[error("no <trkpt> elements found in GPX")]
    NoFixes,

    #[error("track point at index {index} is missing required attribute(s): {reason}")]
    MissingAttribute { index: usize, reason: String },

    #[error("malformed coordinate at index {index}: {reason}")]
    BadCoord { index: usize, reason: String },

    #[error("malformed timestamp at index {index}: {reason}")]
    BadTime { index: usize, reason: String },
}
