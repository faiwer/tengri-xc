//! KML parser. Accepts three flavors: GpsDumpAndroid track Placemarks,
//! standard `<gx:Track>` documents, and GPSBabel/OGR `track_points`
//! fix streams. See `parser` for details.

mod error;
mod parser;

pub use error::KmlError;
pub use parser::{parse_bytes, parse_str};
