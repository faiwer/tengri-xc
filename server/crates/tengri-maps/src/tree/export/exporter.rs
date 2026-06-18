use std::io::Write;
use std::path::PathBuf;
use std::thread;

use crate::tree::TileTreeError;

use super::adapter::{TileTreeExportAdapter, TileTreeExportReport};
use super::orchestrator::Orchestrator;

/// High-level entry point for producing a `.tengri-dem` archive from a
/// [`TileTreeExportAdapter`]. Wraps [`Orchestrator`] in a builder shape so the
/// existing call sites (`DemTree::builder`, the CLI) don't need to know about
/// the orchestrator.
pub struct TileTreeExporter<A> {
    adapter: A,
    destination: PathBuf,
    threads: usize,
    /// Shallowest zoom recorded in the archive. Below this the tree is empty —
    /// the reader returns `TileOutOfBounds` for any tile at `z < min_zoom`.
    min_zoom: u8,
    /// Optional cap on the deepest stored zoom. `None` keeps the source's
    /// native leaf zoom (`source.tile_bounds().zoom`); `Some(z)` re-projects
    /// the tile-space bounds to zoom `z` so the export stops short of the
    /// source's true leaf. Must be `<= source.tile_bounds().zoom`.
    max_zoom: Option<u8>,
    progress: Option<Box<dyn Write + Send>>,
}

impl<A: TileTreeExportAdapter> TileTreeExporter<A> {
    pub fn new(adapter: A, destination: impl Into<PathBuf>) -> Self {
        Self {
            adapter,
            destination: destination.into(),
            threads: default_thread_count(),
            min_zoom: 0,
            max_zoom: None,
            progress: None,
        }
    }

    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = threads.max(1);
        self
    }

    pub fn min_zoom(mut self, min_zoom: u8) -> Self {
        self.min_zoom = min_zoom;
        self
    }

    pub fn max_zoom(mut self, max_zoom: u8) -> Self {
        self.max_zoom = Some(max_zoom);
        self
    }

    pub fn progress(mut self, writer: impl Write + Send + 'static) -> Self {
        self.progress = Some(Box::new(writer));
        self
    }

    pub fn build(self) -> Result<TileTreeExportReport, TileTreeError> {
        let orchestrator = Orchestrator::new(
            self.adapter,
            self.destination,
            self.threads,
            self.min_zoom,
            self.max_zoom,
            self.progress,
        )?;
        orchestrator.run()
    }
}

fn default_thread_count() -> usize {
    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
}
