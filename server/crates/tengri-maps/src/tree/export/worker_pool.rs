//! N-thread worker pool used by the DFS orchestrator.
//!
//! Each worker owns its own `adapter.open_reader()` for the lifetime of the
//! pool. The orchestrator dispatches per-tile [`Job`]s round-robin onto
//! per-worker bounded mpsc channels and drains a single shared result channel.
//! Workers handle two job kinds:
//!
//! - [`Job::Source`] — read a tile via
//!   [`TileTreeExportAdapter::read_source_tile`] and encode it.
//! - [`Job::Reduce`] — deserialise up to 4 children from
//!   [`super::raw_cache::LoadedBlock`] slices, call
//!   [`TileTreeExportAdapter::reduce_children_to_tile`], encode the result.
//!
//! Both paths return `(slot_in_block, encoded, raw)` so the orchestrator can
//! write tile-data in slot order *and* hand the raw on to the parent reduce
//! path (via [`super::raw_cache::RawCache::put`]).
//!
//! Round-robin keeps the dispatch path simple. Tile encoding cost is roughly
//! uniform per source format; if a future archive shows bad stragglers we can
//! swap to a shared work-stealing deque without changing the orchestrator's
//! `fanout_*` API.

use std::sync::Arc;
use std::sync::mpsc;
use std::thread;

use crate::geo::XyzTile;
use crate::tree::error::TileTreeError;

use super::adapter::{CachedChild, TileTreeExportAdapter};
use super::raw_cache::LoadedBlock;

enum Job {
    Source {
        slot_in_block: u32,
        tile: XyzTile,
    },
    Reduce {
        slot_in_block: u32,
        parent_tile: XyzTile,
        children: [Option<ChildLoc>; 4],
    },
    /// Marker so a worker can drain its queue and exit on shutdown.
    Stop,
}

#[derive(Clone)]
pub(super) struct ChildLoc {
    pub(super) child_tile: XyzTile,
    pub(super) loaded: LoadedBlock,
    pub(super) slot_in_child: u32,
}

pub(super) struct JobResult<T> {
    pub(super) slot_in_block: u32,
    /// Compressed tile payload.
    pub(super) encoded: Vec<u8>,
    /// Raw tile payload. `Some` if the pool was built with `keep_raws == true`
    /// (i.e. the orchestrator's raw cache is live).
    pub(super) raw: Option<T>,
}

pub(super) struct WorkerPool<A: TileTreeExportAdapter> {
    workers: Vec<WorkerHandle>,
    result_rx: mpsc::Receiver<Result<JobResult<A::SourceTile>, TileTreeError>>,
    next_worker: usize,
}

struct WorkerHandle {
    job_tx: mpsc::Sender<Job>,
    join: Option<thread::JoinHandle<Result<(), TileTreeError>>>,
}

impl<A: TileTreeExportAdapter> WorkerPool<A> {
    pub(super) fn new(
        adapter: Arc<A>,
        worker_count: usize,
        keep_raws: bool,
    ) -> Result<Self, TileTreeError> {
        let worker_count = worker_count.max(1);
        let (result_tx, result_rx) = mpsc::channel();
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let (job_tx, job_rx) = mpsc::channel::<Job>();
            let adapter = Arc::clone(&adapter);
            let result_tx = result_tx.clone();
            let join = thread::spawn(move || run_worker(adapter, job_rx, result_tx, keep_raws));
            workers.push(WorkerHandle {
                job_tx,
                join: Some(join),
            });
        }
        Ok(Self {
            workers,
            result_rx,
            next_worker: 0,
        })
    }

    /// Dispatch a batch of source-path jobs and collect their results in slot
    /// order. Returns one [`JobResult`] per job, sorted by `slot_in_block`.
    pub(super) fn fanout_source(
        &mut self,
        tiles: &[(u32, XyzTile)],
    ) -> Result<Vec<JobResult<A::SourceTile>>, TileTreeError> {
        for &(slot, tile) in tiles {
            let job = Job::Source {
                slot_in_block: slot,
                tile,
            };
            self.dispatch(job)?;
        }
        self.drain(tiles.len())
    }

    /// Dispatch a batch of reduce jobs and collect their results in slot
    /// order. Each entry is `(slot_in_block, parent_tile, children)`.
    pub(super) fn fanout_reduce(
        &mut self,
        jobs: Vec<(u32, XyzTile, [Option<ChildLoc>; 4])>,
    ) -> Result<Vec<JobResult<A::SourceTile>>, TileTreeError> {
        let count = jobs.len();
        for (slot, parent_tile, children) in jobs {
            let job = Job::Reduce {
                slot_in_block: slot,
                parent_tile,
                children,
            };
            self.dispatch(job)?;
        }
        self.drain(count)
    }

    /// Dispatch a job to a worker. Round-robin to keep the dispatch path simple.
    fn dispatch(&mut self, job: Job) -> Result<(), TileTreeError> {
        let n = self.workers.len();
        let idx = self.next_worker % n;
        self.next_worker = (self.next_worker + 1) % n;
        self.workers[idx]
            .job_tx
            .send(job)
            .map_err(|_| TileTreeError::CorruptFile("worker pool job channel closed"))
    }

    /// Drain the result channel and return the results in slot order.
    fn drain(&mut self, count: usize) -> Result<Vec<JobResult<A::SourceTile>>, TileTreeError> {
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let result = self
                .result_rx
                .recv()
                .map_err(|_| TileTreeError::CorruptFile("worker pool result channel closed"))?;
            out.push(result?);
        }
        out.sort_by_key(|r| r.slot_in_block);
        Ok(out)
    }

    pub(super) fn shutdown(mut self) -> Result<(), TileTreeError> {
        for worker in &self.workers {
            let _ = worker.job_tx.send(Job::Stop);
        }
        for worker in self.workers.iter_mut() {
            if let Some(join) = worker.join.take() {
                join.join().map_err(|_| TileTreeError::WorkerPanicked)??;
            }
        }
        Ok(())
    }
}

impl<A: TileTreeExportAdapter> Drop for WorkerPool<A> {
    fn drop(&mut self) {
        // Best-effort: tell workers to stop. shutdown() on the happy path
        // already joined them; this catches early-return error paths.
        for worker in &self.workers {
            let _ = worker.job_tx.send(Job::Stop);
        }
        for worker in self.workers.iter_mut() {
            if let Some(join) = worker.join.take() {
                let _ = join.join();
            }
        }
    }
}

fn run_worker<A: TileTreeExportAdapter>(
    adapter: Arc<A>,
    job_rx: mpsc::Receiver<Job>,
    result_tx: mpsc::Sender<Result<JobResult<A::SourceTile>, TileTreeError>>,
    keep_raws: bool,
) -> Result<(), TileTreeError> {
    let mut reader = adapter.open_reader()?;
    while let Ok(job) = job_rx.recv() {
        match job {
            Job::Stop => return Ok(()),
            Job::Source {
                slot_in_block,
                tile,
            } => {
                let result = handle_source(
                    adapter.as_ref(),
                    &mut reader,
                    slot_in_block,
                    tile,
                    keep_raws,
                );
                if result_tx.send(result).is_err() {
                    return Ok(());
                }
            }
            Job::Reduce {
                slot_in_block,
                parent_tile,
                children,
            } => {
                let result = handle_reduce(
                    adapter.as_ref(),
                    slot_in_block,
                    parent_tile,
                    &children,
                    keep_raws,
                );
                if result_tx.send(result).is_err() {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

fn handle_source<A: TileTreeExportAdapter>(
    adapter: &A,
    reader: &mut A::Reader,
    slot_in_block: u32,
    tile: XyzTile,
    keep_raws: bool,
) -> Result<JobResult<A::SourceTile>, TileTreeError> {
    let raw = adapter.read_source_tile(reader, tile)?;
    let encoded = adapter.encode_payload(&raw)?;
    let raw = if keep_raws { Some(raw) } else { None };
    Ok(JobResult {
        slot_in_block,
        encoded,
        raw,
    })
}

fn handle_reduce<A: TileTreeExportAdapter>(
    adapter: &A,
    slot_in_block: u32,
    parent_tile: XyzTile,
    children: &[Option<ChildLoc>; 4],
    keep_raws: bool,
) -> Result<JobResult<A::SourceTile>, TileTreeError> {
    let mut cached: Vec<CachedChild<A::SourceTile>> = Vec::with_capacity(4);
    for slot_opt in children {
        let Some(loc) = slot_opt else { continue };
        let bytes = loc.loaded.slot_bytes(loc.slot_in_child)?;
        let mut slice = bytes;
        let raw = adapter.read_raw_cache(&mut slice)?;
        cached.push(CachedChild {
            tile: loc.child_tile,
            raw,
        });
    }
    if cached.is_empty() {
        return Err(TileTreeError::CorruptFile("reduce job has no child tiles"));
    }
    let raw = adapter.reduce_children_to_tile(parent_tile, &cached)?;
    drop(cached);
    let encoded = adapter.encode_payload(&raw)?;
    let raw = if keep_raws { Some(raw) } else { None };
    Ok(JobResult {
        slot_in_block,
        encoded,
        raw,
    })
}
