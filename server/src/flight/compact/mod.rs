//! Compact, delta-encoded representation of a [`Track`].
//!
//! Designed for database storage and wire transfer. ~3× smaller than gzipped
//! IGC; decodable in a single pass (see [`decode`]).
//!
//! [`Track`]: crate::flight::Track

mod decode;
mod encode;
mod error;
mod types;

pub use decode::decode;
pub use encode::encode;
pub use error::CompactError;
pub use types::{CompactTrack, CoordDual, CoordGps, FixDual, FixGps, TimeFix, TrackBody};
