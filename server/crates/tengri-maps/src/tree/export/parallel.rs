use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use crate::geo::XyzTile;
use crate::tree::{TileTreeError, XYZBounds};

use super::adapter::TileTreeExportAdapter;
use super::cache::RawTileCache;
use super::progress::{ProgressWriter, update_progress};
use super::subtree::{add_payload, export_subtree};
use super::super::writer::TileTreeFile;

enum WorkerMessage<T> {
    // Emitted when a tile's payload is ready to be written to the tree file.
    Payload { tile: XyzTile, payload: Vec<u8> },
    // Emitted when a tile's raw tile is ready to be written to the cache file.
    // Used for the top part of each worker's subtree.
    FrontierRaw { tile: XyzTile, raw: T },
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_split_subtrees<A: TileTreeExportAdapter>(
    adapter: Arc<A>,
    bounds: XYZBounds,
    split_zoom: u8,
    worker_count: usize,
    cache: &RawTileCache,
    tree: &mut TileTreeFile,
    written: &mut usize,
    progress: &mut Option<ProgressWriter>,
) -> Result<(), TileTreeError> {
    let tiles_at_split_zoom = Arc::new(bounds.tiles_at(split_zoom)?);
    let next = Arc::new(AtomicUsize::new(0));
    let cancel = Arc::new(AtomicBool::new(false));
    // No more than 8 queued messages per worker.
    let channel_capacity = worker_count.max(1) * 8;
    let (send, receive) = mpsc::sync_channel(channel_capacity);
    let mut workers = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let adapter = Arc::clone(&adapter);
        let tiles = Arc::clone(&tiles_at_split_zoom);
        let next = Arc::clone(&next);
        let cancel = Arc::clone(&cancel);
        let send = send.clone();
        workers.push(thread::spawn(move || {
            gen_split_worker(adapter, bounds, tiles, next, cancel, send)
        }));
    }
    drop(send);

    let mut first_error = None;
    for message in receive {
        match message {
            Ok(WorkerMessage::Payload { tile, payload }) if first_error.is_none() => {
                add_payload(tree, tile, &payload)?;
                *written += 1;
                update_progress(progress, *written);
            }
            Ok(WorkerMessage::FrontierRaw { tile, raw }) if first_error.is_none() => {
                // Write the raw tile to the cache file. It'll be used to
                // compute the parent tiles (that are above the split zoom
                // level)
                cache.write(adapter.as_ref(), tile, &raw)?;
                drop(raw);
            }
            Ok(_) => {}
            Err(error) => {
                if first_error.is_none() {
                    cancel.store(true, Ordering::Relaxed);
                    first_error = Some(error);
                }
            }
        }
    }

    for worker in workers {
        match worker.join().map_err(|_| TileTreeError::WorkerPanicked)? {
            Ok(()) => {}
            Err(error) if first_error.is_none() => {
                first_error = Some(error);
            }
            Err(_) => {}
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(())
}

fn gen_split_worker<A: TileTreeExportAdapter>(
    adapter: Arc<A>,
    bounds: XYZBounds,
    tiles: Arc<Vec<XyzTile>>,
    next: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    send: mpsc::SyncSender<Result<WorkerMessage<A::SourceTile>, TileTreeError>>,
) -> Result<(), TileTreeError> {
    let mut reader = adapter.open_reader()?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let idx = next.fetch_add(1, Ordering::Relaxed);
        let Some(&tile) = tiles.get(idx) else {
            return Ok(());
        };

        let result = export_subtree(
            adapter.as_ref(),
            &mut reader,
            bounds,
            tile,
            // We need the raw tile to compute the parent tiles (that are above
            // the split zoom level)
            true,
            &mut |tile, payload| {
                // Send the payload to the main thread to be written to the tree file.
                send.send(Ok(WorkerMessage::Payload { tile, payload }))
                    .map_err(|_| TileTreeError::CorruptFile("tile result receiver closed"))
            },
        );
        match result {
            Ok(Some(raw)) => {
                send.send(Ok(WorkerMessage::FrontierRaw { tile, raw }))
                    .map_err(|_| TileTreeError::CorruptFile("tile result receiver closed"))?;
            }
            Ok(None) => {
                send.send(Err(TileTreeError::CorruptFile(
                    "split root did not return raw tile",
                )))
                .map_err(|_| TileTreeError::CorruptFile("tile result receiver closed"))?;
                return Ok(());
            }
            Err(error) => {
                send.send(Err(error))
                    .map_err(|_| TileTreeError::CorruptFile("tile result receiver closed"))?;
                return Ok(());
            }
        }
    }
}
