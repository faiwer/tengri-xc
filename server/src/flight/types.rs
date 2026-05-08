//! In-memory flight track. Array-of-structs layout, full source precision
//! preserved for analytics.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    /// Unix epoch seconds at the first point. Cached for convenience;
    /// equals `points[0].time`.
    pub start_time: u32,

    /// Track samples in chronological order.
    pub points: Vec<TrackPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackPoint {
    /// Unix epoch seconds.
    pub time: u32,

    /// Latitude in E5 micro-degrees (deg × 10⁵). One unit ≈ 1.1 m at the
    /// equator. Range: ±90 × 10⁵ = ±9 000 000.
    pub lat: i32,

    /// Longitude in E5 micro-degrees (deg × 10⁵). One unit ≈ 1.1 m at the
    /// equator. Range: ±180 × 10⁵ = ±18 000 000.
    pub lon: i32,

    /// GPS / geodetic altitude in decimeters (m × 10). 0.1 m resolution,
    /// ~±214 km range.
    pub geo_alt: i32,

    /// Barometric pressure altitude in decimeters. `None` for tracks
    /// recorded without a barometer (e.g. handheld GPS-only files).
    /// Within a single track this is either `Some` for every point or
    /// `None` for every point — mixing is rejected by the encoder.
    pub pressure_alt: Option<i32>,

    /// True airspeed in km/h, integer. `None` for tracks whose source
    /// has no TAS channel (most files in the wild). Within a single
    /// track this is either `Some` for every point or `None` for
    /// every point — mismatched-length input is rejected upstream
    /// (parser drops the channel rather than partially populating).
    /// Sourced today only from IGC files with an `I…TAS` extension.
    pub tas: Option<u16>,
}
