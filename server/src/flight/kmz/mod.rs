//! KMZ = ZIP-wrapped KML. Thin façade: unzip → reuse the KML parser.

mod error;
mod parser;

pub use error::KmzError;
pub use parser::{extract_kml_bytes, parse_bytes};
