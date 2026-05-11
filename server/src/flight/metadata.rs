//! Off-track metadata that travels with a flight in a `.tengri` envelope.
//!
//! Deliberately a sibling of [`CompactTrack`](super::compact::CompactTrack),
//! never nested inside it: the compact format stays strictly about time and
//! geometry; this struct is the wrapper for everything else we want to keep
//! near the track without folding into the time/space arrays.
//!
//! Bincode is positional, so any change to field order or set requires a
//! [`super::tengri::VERSION`] bump. The four `_lat` / `_lon` fields are
//! E5 micro-degrees (deg × 10⁵), matching [`super::types::TrackPoint`]'s
//! coordinate units exactly so callers can pull them straight off the
//! takeoff/landing fix.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    /// UTC offset in whole seconds at the takeoff fix, computed via the
    /// flight's first flying coordinate and `tzdb` rules valid on the
    /// flight's date. Positive = ahead of UTC.
    pub takeoff_offset: i32,
    pub landing_offset: i32,
    pub takeoff_lat: i32,
    pub takeoff_lon: i32,
    pub landing_lat: i32,
    pub landing_lon: i32,
}
