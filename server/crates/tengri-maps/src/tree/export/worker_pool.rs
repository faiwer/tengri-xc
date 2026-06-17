
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;

use crate::geo::XyzTile;
use crate::tree::error::TileTreeError;

use super::adapter::{CachedChild, TileTreeExportAdapter};
use super::raw_cache::LoadedBlock;

enum Job {
    Source {
    },
    Reduce {
    },
    /// Marker so a worker can drain its queue and exit on shutdown.
    Stop,
}

pub(super) struct JobResult<T> {
}

pub(super) struct WorkerPool<A: TileTreeExportAdapter> {
}

struct WorkerHandle {
}

impl<A: TileTreeExportAdapter> WorkerPool<A> {
    pub(super) fn new(adapter: Arc<A>, worker_count: usize) -> Result<Self, TileTreeError> {
        Ok(Self {
        })
    }

    pub(super) fn fanout_source(
    ) -> Result<Vec<JobResult<A::SourceTile>>, TileTreeError> {
    pub(super) fn fanout_reduce(
    ) -> Result<Vec<JobResult<A::SourceTile>>, TileTreeError> {
    }

    fn dispatch(&mut self, job: Job) -> Result<(), TileTreeError> {
    }

    fn drain(
        &mut self,
    ) -> Result<Vec<JobResult<A::SourceTile>>, TileTreeError> {
    }

    pub(super) fn shutdown(mut self) -> Result<(), TileTreeError> {
    }
}

impl<A: TileTreeExportAdapter> Drop for WorkerPool<A> {
    fn drop(&mut self) {
    }
}

fn run_worker<A: TileTreeExportAdapter>(
) -> Result<(), TileTreeError> {
    Ok(())
}

fn handle_source<A: TileTreeExportAdapter>(
) -> Result<JobResult<A::SourceTile>, TileTreeError> {
}

fn handle_reduce<A: TileTreeExportAdapter>(
    children: &[Option<ChildLoc>; 4],
) -> Result<JobResult<A::SourceTile>, TileTreeError> {
