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

    /// True airspeed channel. `TasBody::None` whenever the source had no
    /// TAS (most paragliding loggers; phone apps; KML/GPX inputs that
    /// lack a 1:1 airspeed extension). When present, the body uses the
    /// same fix+delta merge-walk as `TrackBody`.
    pub tas: TasBody,

    /// FNV-1a 32 over the deterministic byte stream of all preceding fields,
    /// in declaration order. Lets the FE verify post-decode that the wire
    /// payload survived transport and that both ends agree on the parser
    /// shape. Computed by [`super::hash::compute`]; verified by the FE.
    pub hash: u32,
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

/// True airspeed channel. Mirrors the `TrackBody::Gps`/`Dual` pattern:
/// either there's no TAS data at all, or there's a fix+delta pair that
/// reconstructs a per-fix `u16` km/h value. The channel is all-or-
/// nothing per track — the source format must have produced one TAS
/// reading for every fix; partial coverage is rejected upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TasBody {
    /// No TAS data available. Decoder sets `TrackPoint.tas = None` on
    /// every point.
    None,

    /// Per-fix TAS. `fixes` holds sparse absolute-state overrides
    /// (must include `idx=0`); `deltas` holds the per-non-fix-index
    /// `i8` step in km/h. The decoder merges them at decode time so
    /// `fixes.len() + deltas.len() == total_points`.
    Tas { fixes: Vec<TasFix>, deltas: Vec<i8> },
}

/// Sparse absolute-state override for the TAS channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TasFix {
    pub idx: u32,
    /// Absolute TAS in km/h. `u16` covers sailplanes (VNE ~285 km/h);
    /// `u8` would clip them. The extra byte over `u8` is amortized
    /// across the whole flight since the fix list stays sparse —
    /// hang gliders and paragliders need only one entry at idx=0,
    /// sailplanes add a handful of overrides on rapid speed changes.
    pub tas: u16,
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
