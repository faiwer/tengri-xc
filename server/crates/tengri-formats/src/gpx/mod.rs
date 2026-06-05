//! GPX 1.0 / 1.1 parser. Accepts `<trk><trkseg><trkpt>` shapes;
//! ignores routes, waypoints, and extension payloads. See `parser`
//! for details.

mod error;
mod parser;

pub use error::GpxError;
pub use parser::{parse_bytes, parse_str};
