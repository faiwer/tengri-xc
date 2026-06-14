//! Dimension-aware operations on dense 2D grids.
//!
//! The functions here are deliberately ignorant of elevation, tiling, or
//! geography — they operate on row-major pixel buffers with explicit
//! `width`/`height`. Callers layer their own clamps and rounding on top.

mod resample;

pub use resample::area_resample;
