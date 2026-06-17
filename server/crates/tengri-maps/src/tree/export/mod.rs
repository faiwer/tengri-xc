//! Exporter from a [`TileTreeExportAdapter`] source to a `.tengri-dem` archive.
//!
//! - [`adapter`] — the [`TileTreeExportAdapter`] trait callers implement to
//!   plug a source format into the exporter.
//! - [`exporter`] — thin builder over [`orchestrator::Orchestrator`].
//! - [`orchestrator`] — DFS that walks the block grid bottom-up, fans every
//!   block's tiles out to [`worker_pool::WorkerPool`], and writes encoded
//!   tile-data plus the 16 KiB envelope to the destination directly.
//! - [`worker_pool`] — N-thread pool, one source reader per worker, mpsc
//!   round-robin dispatch.
//! - [`raw_cache`] — per-block raw-tile cache spilled to `temp_dir` between
//!   children DFS and parent reduce.
//! - [`encode`] — per-block size-stream encoding and 16 KiB envelope assembly.
//! - [`pack_extras`] — end-pass that fills each envelope's leftover headroom
//!   with neighbour self-payloads.
//! - [`progress`] — optional progress-line writer with a rolling-window ETA.

mod adapter;
mod encode;
mod exporter;
mod orchestrator;
mod pack_extras;
mod progress;
mod raw_cache;
#[cfg(test)]
mod tests;
mod worker_pool;

pub use adapter::{CachedChild, TileTreeExportAdapter, TileTreeExportReport};
pub use exporter::TileTreeExporter;
