pub mod compact;
pub(crate) mod geo_text;
pub mod gpx;
pub mod igc;
pub mod ingest;
pub mod kml;
pub mod kmz;
pub mod metadata;
pub mod tengri;
pub mod types;
pub mod window;

pub use compact::{CompactError, CompactTrack, decode, encode};
pub use gpx::GpxError;
pub use igc::IgcError;
pub use ingest::{
    InputFormat, detect_format, normalize_for_storage, parse_format, parse_input,
    slice_flight_window, slice_time_range,
};
pub use kml::KmlError;
pub use kmz::KmzError;
pub use metadata::Metadata;
pub use tengri::{TengriError, TengriFile};
pub use types::{Track, TrackPoint};
pub use window::{FlightWindow, find_flight_window};
