//! Off-track metadata that travels with a flight in a `.tengri` envelope.
//!
//! Deliberately a sibling of [`CompactTrack`](super::compact::CompactTrack),
//! never nested inside it: the compact format stays strictly about time and
//! geometry; this struct is the wrapper for everything else we want to keep
//! near the track without folding into the time/space arrays.
//!
//! Bincode is positional, so any change to field order or set requires a
//! [`super::tengri::VERSION`] bump. The four `_lat` / `_lon` fields are E5
//! micro-degrees (deg × 10⁵), matching [`super::types::TrackPoint`]'s
//! coordinate units exactly so callers can pull them straight off the
//! takeoff/landing fix.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    /// IANA timezone at the takeoff fix.
    pub takeoff_timezone: String,
    pub landing_timezone: String,
    pub takeoff_lat: i32,
    pub takeoff_lon: i32,
    pub landing_lat: i32,
    pub landing_lon: i32,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            takeoff_timezone: "Etc/UTC".to_owned(),
            landing_timezone: "Etc/UTC".to_owned(),
            takeoff_lat: 0,
            takeoff_lon: 0,
            landing_lat: 0,
            landing_lon: 0,
        }
    }
}
