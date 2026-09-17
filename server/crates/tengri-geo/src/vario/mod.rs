//! Vertical velocity: the smoothed per-fix slope, its 1 m/s quantisation, and
//! the run merging that turns it into something worth drawing. Mirrors the
//! client's `track/varioSegments` so both sides colour a track the same way.

mod buckets;
mod interval;
mod segments;
mod slope;

#[cfg(test)]
mod tests;

pub use buckets::{MAX_BUCKET, MIN_BUCKET, classify_buckets};
pub use interval::{MAX_VARIO_FIX_INTERVAL_SECONDS, average_fix_interval};
pub use segments::{VarioSegment, build_vario_segments};
pub use slope::{VARIO_WINDOW_HALF_SECONDS, vario_mps};
