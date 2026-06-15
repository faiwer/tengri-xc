use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use crate::tree::{SlotIndex, TileTreeError, XYZBounds};

use super::adapter::{TileTreeExportAdapter, TileTreeExportReport};
use super::cache::RawTileCache;
use super::parallel::write_split_subtrees;
use super::progress::{ProgressWriter, update_progress};
use super::reduce::reduce_cached;
use super::subtree::{add_payload, export_subtree};
use super::super::writer::TileTreeFile;

pub struct TileTreeExporter<A> {
    adapter: A,
    destination: PathBuf,
    threads: usize,
    progress: Option<Box<dyn Write + Send>>,
}

impl<A: TileTreeExportAdapter> TileTreeExporter<A> {
    pub fn new(adapter: A, destination: impl Into<PathBuf>) -> Self {
        Self {
            adapter,
            destination: destination.into(),
            threads: default_thread_count(),
            progress: None,
        }
    }

    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = threads.max(1);
        self
    }

    pub fn progress(mut self, writer: impl Write + Send + 'static) -> Self {
        self.progress = Some(Box::new(writer));
        self
    }

    pub fn build(mut self) -> Result<TileTreeExportReport, TileTreeError> {
        let adapter = Arc::new(self.adapter);
        let bounds = adapter.bounds();
        let total = usize::try_from(SlotIndex::new(bounds)?.total_entries())
            .map_err(|_| TileTreeError::InvalidBounds("index is too large"))?;
        let split_zoom = split_zoom(bounds, self.threads)?;
        let mut progress = self
            .progress
            .take()
            .map(|writer| ProgressWriter::new(writer, total));
        let mut written = 0;
        let mut tree = TileTreeFile::create(&self.destination)
            .tile_kind(adapter.tile_kind())
            .bounds(bounds)
            // An empty header for now, will be filled later.
            .finish_header()?;

        if split_zoom == 0 {
            // Can't use parallelization at all.
            let mut reader = adapter.open_reader()?;
            for tile in bounds.tiles_at(0)? {
                export_subtree(
                    adapter.as_ref(),
                    &mut reader,
                    bounds,
                    tile,
                    false,
                    &mut |tile, payload| {
                        add_payload(&mut tree, tile, &payload)?;
                        written += 1;
                        update_progress(&mut progress, written);
                        Ok(())
                    },
                )?;
            }
        } else {
            let cache = RawTileCache::new(&self.destination)?;
            write_split_subtrees(
                Arc::clone(&adapter),
                bounds,
                split_zoom,
                self.threads,
                &cache,
                &mut tree,
                &mut written,
                &mut progress,
            )?;

            for tile in bounds.tiles_at(0)? {
                let raw = reduce_cached(
                    adapter.as_ref(),
                    &cache,
                    bounds,
                    tile,
                    split_zoom,
                    &mut |tile, payload| {
                        add_payload(&mut tree, tile, &payload)?;
                        written += 1;
                        update_progress(&mut progress, written);
                        Ok(())
                    },
                )?;
                drop(raw);
            }
        }

        tree.finish()?;
        if let Some(progress) = progress.as_mut() {
            progress.finish();
        }

        Ok(TileTreeExportReport {
            zoom: bounds.zoom,
            tiles_written: written,
        })
    }
}

/// Returns the zoom level at which the tree should be split into subtrees to 
/// have at least `worker_count` subtrees.
pub(super) fn split_zoom(bounds: XYZBounds, worker_count: usize) -> Result<u8, TileTreeError> {
    for z in 0..=bounds.zoom {
        if bounds.level_bounds(z)?.xy_tiles_count() >= worker_count as u64 {
            return Ok(z);
        }
    }
    Ok(bounds.zoom)
}

fn default_thread_count() -> usize {
    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
}
