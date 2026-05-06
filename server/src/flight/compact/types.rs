//! Locked compact-format types. See [`crate::flight::compact`] for the
//! encoder/decoder algorithms.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactTrack {
    /// Unix epoch seconds at fix #0.
    pub start_time: u32,

    /// Median sample period in seconds (typically 1 for paragliding).
    /// Used to reconstruct timestamps for points without a `TimeFix`.
    pub interval: u16,

    /// GPS-only or dual-altitude body. The variant carries the homogeneous
    /// `Vec`s of fixes and coords; this avoids a per-element discriminant.
    pub track: TrackBody,

    /// Sparse timestamp overrides at indices where the actual Δt deviated
    /// from `interval` (encoder rule: |Δt − interval| big enough to matter,
    /// concretely Δt > 1.5 × interval). Strictly increasing by `idx`.
    pub time_fixes: Vec<TimeFix>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackBody {
    /// GPS-only track (no barometer).
    Gps {
        /// Sparse absolute-state overrides. Must contain a fix at `idx=0`
        /// and be strictly increasing in `idx`. An entry is emitted whenever
        /// any of lat / lon / geo_alt would overflow `i8` as a delta.
        fixes: Vec<FixGps>,
        /// Per-point delta. There is exactly one entry per non-fix index,
        /// so `fixes.len() + coords.len() == total_points`.
        coords: Vec<CoordGps>,
    },
    /// Dual-altitude track (GPS + barometric).
    Dual {
        fixes: Vec<FixDual>,
        coords: Vec<CoordDual>,
    },
}

/// Sparse absolute-state override (GPS-only track).
///
/// Emitted on any-axis overflow. The first fix at `idx=0` is mandatory and
/// provides the initial decoder state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixGps {
    pub idx: u32,
    /// E5 micro-degrees (≈ 1.1 m).
    pub lat: i32,
    pub lon: i32,
    /// Decimeters (0.1 m).
    pub geo_alt: i32,
}

/// Sparse absolute-state override (dual-altitude track).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixDual {
    pub idx: u32,
    /// E5 micro-degrees.
    pub lat: i32,
    pub lon: i32,
    /// Decimeters.
    pub geo_alt: i32,
    pub pressure_alt: i32,
}

/// Per-point delta (GPS-only track). 3 bytes packed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordGps {
    /// Delta in E5 micro-degree units (≈ ±140 m at i8 limits).
    pub lat: i8,
    pub lon: i8,
    /// Delta in decimeters per sample (≈ ±12.7 m/s at 1 Hz).
    pub geo_alt: i8,
}

/// Per-point delta (dual-altitude track). 4 bytes packed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordDual {
    pub lat: i8,
    pub lon: i8,
    pub geo_alt: i8,
    pub pressure_alt: i8,
}

/// Sparse timestamp override at indices where Δt deviated from `interval`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeFix {
    pub idx: u32,
    /// Absolute Unix epoch seconds at this index.
    pub time: u32,
}

impl CompactTrack {
    /// Total reconstructed point count.
    pub fn len(&self) -> usize {
        match &self.track {
            TrackBody::Gps { fixes, coords } => fixes.len() + coords.len(),
            TrackBody::Dual { fixes, coords } => fixes.len() + coords.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
