//! `.tengri` file format: a framed envelope carrying a [`CompactTrack`] plus
//! sibling metadata. See [`format`] for the on-disk layout.
//!
//! [`CompactTrack`]: super::compact::CompactTrack

mod error;
mod format;

pub use error::TengriError;
pub use format::{MAGIC, TengriFile, VERSION};
