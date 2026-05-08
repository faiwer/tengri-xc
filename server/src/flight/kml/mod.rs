//! KML parser. Accepts two flavors: GpsDumpAndroid track Placemarks and
//! standard `<gx:Track>` documents. See `parser` for details.

mod error;
mod parser;

pub use error::KmlError;
pub use parser::{parse_bytes, parse_str};
