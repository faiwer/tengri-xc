//! Exporting a [`super::TileTreeFile`] from a tile source.
//!
//! - [`adapter`] — the [`TileTreeExportAdapter`] trait callers implement
//!   to plug a source format into the exporter.
//! - [`exporter`] — the [`TileTreeExporter`] builder and its `build()`
//!   driver that picks single- vs. multi-threaded paths.
//! - [`subtree`] — single-threaded recursion that walks a parent down
//!   to leaves and emits payloads.
//! - [`parallel`] — multi-threaded split-zoom frontier that fans out
//!   subtrees across worker threads.
//! - [`reduce`] — post-frontier reduction that drains cached raw tiles
//!   and emits intermediate parents.
//! - [`cache`] — on-disk cache for raw tiles handed off between the
//!   frontier and the reduction pass.
//! - [`progress`] — optional progress-line writer with a rolling-window
//!   ETA.

mod adapter;
mod cache;
mod exporter;
mod parallel;
mod progress;
mod reduce;
mod subtree;
#[cfg(test)]
mod tests;

pub use adapter::{CachedChild, TileTreeExportAdapter, TileTreeExportReport};
pub use exporter::TileTreeExporter;
