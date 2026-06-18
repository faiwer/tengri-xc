//! DFS orchestrator that walks the block grid bottom-up, fans every block's
//! tiles out to the worker pool, and writes encoded tile-data plus envelope
//! into the destination file directly. After the DFS finishes, a brief end-pass
//! packs neighbour extras into envelopes that have headroom.
//!
//! The orchestrator is the only thread that writes to `dest`. Workers produce
//! `(slot_in_block, encoded, raw)` tuples; the orchestrator drains them in slot
//! order, appends tile bytes at `dest_cursor`, encodes the per-block
//! size-stream, and writes the 16 KiB envelope at its fixed offset.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::Arc;

use super::adapter::{TileTreeExportAdapter, TileTreeExportReport};
use super::encode::{CompressedBlock, SizeStream, build_envelope};
use super::pack_extras;
use super::progress::{ProgressWriter, update_progress};
use super::raw_cache::{LoadedBlock, RawCache};
use super::worker_pool::{ChildLoc, JobResult, WorkerPool};
use crate::geo::XyzTile;
use crate::tree::blocks::{BlockDescriptor, BlockGrid};
use crate::tree::error::TileTreeError;
use crate::tree::format::{BLOCK_SIZE, HEADER_LEN, MAGIC, write_header};
use crate::tree::metadata::TileTreeMetadata;

const ZSTD_LEVEL: i32 = 3;

pub(super) struct Orchestrator<A: TileTreeExportAdapter> {
    adapter: Arc<A>,
    metadata: TileTreeMetadata,
    grid: BlockGrid,
    /// Shallowest stored zoom; DFS roots live here and the spill-to-cache step
    /// stops here.
    min_zoom: u8,
    dest_file: File,
    /// Next byte offset in the tile-data section. DFS appends here.
    dest_cursor: u64,
    /// Start of the tile-data section: `HEADER_LEN + block_count * BLOCK_SIZE`.
    tile_data_off: u64,
    /// Per-block compressed-self-payload byte count. `len_self[i] == 0`
    /// means block `i` hasn't been written yet.
    len_self: Vec<u16>,
    hasher: blake3::Hasher,
    pool: WorkerPool<A>,
    /// Per-block spill of source-direct raws so the reduce path can stitch
    /// parents. `None` when the source supplies every zoom in `bounds()`
    /// directly (e.g. a full PMTiles pyramid) — then every parent will
    /// source-direct too and the cache would just churn disk pointlessly.
    raw_cache: Option<RawCache>,
    tiles_written: usize,
    /// Number of blocks for which `process_block` has produced a write. Drives
    /// the progress reporter; `tiles_written` stays for the final
    /// `TileTreeExportReport`.
    blocks_done: usize,
    progress: Option<ProgressWriter>,
}

impl<A: TileTreeExportAdapter> Orchestrator<A> {
    pub(super) fn new(
        adapter: A,
        destination: impl Into<PathBuf>,
        threads: usize,
        min_zoom: u8,
        max_zoom: Option<u8>,
        progress: Option<Box<dyn Write + Send>>,
    ) -> Result<Self, TileTreeError> {
        let adapter = Arc::new(adapter);
        let source_bounds = adapter.bounds();
        // A `max_zoom` cap re-projects the source bounds to that zoom; the
        // exporter then treats that zoom as its leaf. `level_bounds` validates
        // `max_zoom <= source_bounds.zoom`.
        let bounds = match max_zoom {
            Some(z) => source_bounds.level_bounds(z)?,
            None => source_bounds,
        };
        // For leaf-only sources, the gap between the source's native zoom and
        // the exported leaf is the per-leaf-read downsample factor. Refuse
        // upfront when the adapter advertises a smaller cap than that gap;
        // letting the DFS start would just blow up at the first tile read
        // half-way through, with the destination half-written.
        if !adapter.supplies_all_zooms() {
            let gap = source_bounds.zoom.saturating_sub(bounds.zoom);
            let max_supported_gap = adapter.max_leaf_downsample_steps();
            if gap > max_supported_gap {
                return Err(TileTreeError::LeafZoomGapTooLarge {
                    source_zoom: source_bounds.zoom,
                    requested_zoom: bounds.zoom,
                    max_supported_gap,
                });
            }
        }
        let metadata = TileTreeMetadata::new(adapter.tile_kind(), bounds);
        let grid = BlockGrid::new(bounds, min_zoom)?;
        let block_count = grid.total_blocks();
        let tile_data_off = HEADER_LEN
            .checked_add(
                block_count
                    .checked_mul(BLOCK_SIZE)
                    .ok_or(TileTreeError::InvalidBounds("tile data offset overflow"))?,
            )
            .ok_or(TileTreeError::InvalidBounds("tile data offset overflow"))?;

        let destination: PathBuf = destination.into();
        let dest_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&destination)?;
        // Reserve sparse space for header + block region. DFS streams tile data
        // starting at tile_data_off; envelope writes land in the reserved
        // region.
        dest_file.set_len(tile_data_off)?;

        let total_blocks = usize::try_from(block_count)
            .map_err(|_| TileTreeError::InvalidBounds("block count exceeds usize"))?;
        let progress = progress.map(|writer| ProgressWriter::new(writer, total_blocks));

        let len_self = vec![
            0u16;
            usize::try_from(block_count).map_err(|_| {
                TileTreeError::InvalidBounds("block count exceeds usize")
            })?
        ];
        let raw_cache = if adapter.supplies_all_zooms() {
            None
        } else {
            Some(RawCache::new(&destination)?)
        };
        // Workers only need to ship raw `SourceTile`s back to the orchestrator
        // when there's a cache that consumes them.
        let pool = WorkerPool::new(Arc::clone(&adapter), threads, raw_cache.is_some())?;

        Ok(Self {
            adapter,
            metadata,
            grid,
            min_zoom,
            dest_file,
            dest_cursor: tile_data_off,
            tile_data_off,
            len_self,
            hasher: blake3::Hasher::new(),
            pool,
            raw_cache,
            tiles_written: 0,
            blocks_done: 0,
            progress,
        })
    }

    pub(super) fn run(mut self) -> Result<TileTreeExportReport, TileTreeError> {
        let roots: Vec<BlockDescriptor> = self.grid.blocks_at_zoom(self.min_zoom)?.to_vec();
        for root in &roots {
            self.process_block(root)?;
        }

        let tile_data_len = self.dest_cursor - self.tile_data_off;
        let payload_hash = *self.hasher.finalize().as_bytes();

        pack_extras::run(&self.dest_file, &self.grid, &self.len_self)?;

        self.dest_file
            .write_all_at(&MAGIC, self.tile_data_off + tile_data_len)?;
        let mut header_buf: Vec<u8> = Vec::with_capacity(HEADER_LEN as usize);
        write_header(
            &mut header_buf,
            self.metadata,
            self.min_zoom,
            tile_data_len,
            payload_hash,
        )?;
        self.dest_file.write_all_at(&header_buf, 0)?;
        self.dest_file.sync_all()?;

        if let Some(progress) = self.progress.as_mut() {
            progress.finish();
        }

        let pool = self.pool;
        let _ = self.raw_cache;
        pool.shutdown()?;

        Ok(TileTreeExportReport {
            zoom: self.metadata.bounds.zoom,
            tiles_written: self.tiles_written,
        })
    }

    fn process_block(&mut self, block: &BlockDescriptor) -> Result<(), TileTreeError> {
        // Children must land in the archive regardless of whether this
        // block's tiles come from source or from reducing children, so we
        // always descend at non-leaf zooms. The reduce path additionally
        // pulls their raws back out of the cache; the source path simply
        // ignores them and the cache cleans up at end.
        let children: Vec<BlockDescriptor> = if block.zoom < self.metadata.bounds.zoom {
            self.grid.children_of(block).into_iter().copied().collect()
        } else {
            Vec::new()
        };
        for child in &children {
            self.process_block(child)?;
        }

        // The exported leaf is `metadata.bounds.zoom`, which is the source's
        // native leaf or a `max_zoom` cap below it. Either way it's the
        // deepest zoom the orchestrator will ask the source for, so that's
        // the right comparison for the leaf-zoom branch — even when the cap
        // is shallower than what the source could provide.
        let source_supplies =
            self.adapter.supplies_all_zooms() || block.zoom == self.metadata.bounds.zoom;

        let results = if source_supplies {
            self.run_source_path(block)?
        } else {
            if children.is_empty() {
                return Err(TileTreeError::MissingTile {
                    z: block.zoom,
                    x: block.origin_x,
                    y: block.origin_y,
                });
            }
            self.run_reduce_path(block, &children)?
        };

        self.write_block(block, &results)?;
        self.tiles_written += results.len();
        self.blocks_done += 1;
        update_progress(&mut self.progress, self.blocks_done);

        if block.zoom > self.min_zoom {
            // cache is none when supplies_all_zooms() is true
            if let Some(cache) = self.raw_cache.as_mut() {
                // Pool was built with `keep_raws == raw_cache.is_some()`, so
                // every result on this branch has its raw populated.
                let raws: Vec<A::SourceTile> = results
                    .into_iter()
                    .map(|r| {
                        r.raw.ok_or_else(|| {
                            TileTreeError::CorruptFile(
                                "worker dropped raw tile but raw cache is live",
                            )
                        })
                    })
                    .collect::<Result<_, _>>()?;
                cache.put(self.adapter.as_ref(), block.zoom, block.block_id, &raws)?;
            }
        }

        Ok(())
    }

    /// The source supplies the raw tiles for this block.
    fn run_source_path(
        &mut self,
        block: &BlockDescriptor,
    ) -> Result<Vec<JobResult<A::SourceTile>>, TileTreeError> {
        let tile_count = block.dims.tile_count() as usize;
        let mut jobs: Vec<(u32, XyzTile)> = Vec::with_capacity(tile_count);
        for dy in 0..block.dims.block_h {
            for dx in 0..block.dims.block_w {
                let slot = u32::from(dy) * u32::from(block.dims.block_w) + u32::from(dx);
                let x = u32::from(block.origin_x + u16::from(dx));
                let y = u32::from(block.origin_y + u16::from(dy));
                jobs.push((
                    slot,
                    XyzTile {
                        z: block.zoom,
                        x,
                        y,
                    },
                ));
            }
        }
        self.pool.fanout_source(&jobs)
    }

    fn run_reduce_path(
        &mut self,
        block: &BlockDescriptor,
        children: &[BlockDescriptor],
    ) -> Result<Vec<JobResult<A::SourceTile>>, TileTreeError> {
        // Reduce path is only reachable when the source could not satisfy at
        // least one block at this zoom. Adapters that report
        // `supplies_all_zooms() == true` never get here, so the cache must
        // exist by the time we need to load children.
        let cache = self.raw_cache.as_mut().ok_or(TileTreeError::CorruptFile(
            "reduce path requires a raw cache; adapter declared supplies_all_zooms",
        ))?;
        let mut loaded: Vec<(u64, LoadedBlock)> = Vec::with_capacity(children.len());
        for child in children {
            let load = cache.load_and_drop(child.block_id)?;
            loaded.push((child.block_id, load));
        }

        let parent_z = block.zoom;
        let child_z = parent_z + 1;
        let tile_count = block.dims.tile_count() as usize;
        let mut jobs: Vec<(u32, XyzTile, [Option<ChildLoc>; 4])> = Vec::with_capacity(tile_count);
        for dy in 0..block.dims.block_h {
            for dx in 0..block.dims.block_w {
                let slot = u32::from(dy) * u32::from(block.dims.block_w) + u32::from(dx);
                let parent_x = block.origin_x + u16::from(dx);
                let parent_y = block.origin_y + u16::from(dy);
                let parent_tile = XyzTile {
                    z: parent_z,
                    x: u32::from(parent_x),
                    y: u32::from(parent_y),
                };

                let mut children_arr: [Option<ChildLoc>; 4] = Default::default();
                let mut idx = 0;
                for cdy in 0..2u32 {
                    for cdx in 0..2u32 {
                        let child_x = u32::from(parent_x) * 2 + cdx;
                        let child_y = u32::from(parent_y) * 2 + cdy;
                        let Ok(child_x_u16) = u16::try_from(child_x) else {
                            continue;
                        };
                        let Ok(child_y_u16) = u16::try_from(child_y) else {
                            continue;
                        };
                        let Ok(loc) = self.grid.block_for(child_z, child_x_u16, child_y_u16) else {
                            continue;
                        };
                        let Some((_, loaded_block)) =
                            loaded.iter().find(|(id, _)| *id == loc.block_id)
                        else {
                            continue;
                        };
                        children_arr[idx] = Some(ChildLoc {
                            child_tile: XyzTile {
                                z: child_z,
                                x: child_x,
                                y: child_y,
                            },
                            loaded: loaded_block.clone(),
                            slot_in_child: loc.slot_in_block,
                        });
                        idx += 1;
                    }
                }
                jobs.push((slot, parent_tile, children_arr));
            }
        }

        let results = self.pool.fanout_reduce(jobs)?;
        // `loaded` drops here, releasing the child Arc<Vec<u8>>s. Disk
        // files were already deleted at load_and_drop time.
        drop(loaded);
        Ok(results)
    }

    fn write_block(
        &mut self,
        block: &BlockDescriptor,
        results: &[JobResult<A::SourceTile>],
    ) -> Result<(), TileTreeError> {
        let tile_count = block.dims.tile_count();
        if results.len() != tile_count as usize {
            return Err(TileTreeError::CorruptFile(
                "write_block result count mismatch",
            ));
        }

        let block_base_offset = self.dest_cursor;
        let mut size_stream = SizeStream::new(tile_count, block_base_offset);
        let mut prev_bytes: Option<&[u8]> = None;

        for (i, result) in results.iter().enumerate() {
            let raw_tile_payload = result.encoded.as_slice();
            let length = u32::try_from(raw_tile_payload.len())
                .map_err(|_| TileTreeError::TileTooLarge(raw_tile_payload.len() as u64))?;

            // We perform anchor reuse when this slot's encoded bytes match the
            // running anchor. After every fresh write the next slot's
            // "previous" is the new anchor; consecutive anchor-reuses don't
            // change the anchor, so comparing to `prev_bytes` (which we only
            // update on a fresh write) implements the same semantics the
            // converter had via blob-key equality.
            let is_anchor_reuse = i > 0 && prev_bytes.map_or(false, |pb| pb == raw_tile_payload);
            if is_anchor_reuse {
                size_stream.push_anchor_reuse()?;
            } else {
                self.dest_file
                    .write_all_at(raw_tile_payload, self.dest_cursor)?;
                self.hasher.update(raw_tile_payload);
                self.dest_cursor = self
                    .dest_cursor
                    .checked_add(u64::from(length))
                    .ok_or(TileTreeError::CorruptFile("dest cursor overflow"))?;
                if i == 0 {
                    size_stream.push_first(length)?;
                } else {
                    size_stream.push_fresh(length)?;
                }
                prev_bytes = Some(raw_tile_payload);
            }
        }

        let stream_bytes = size_stream.finish()?;
        let compressed = CompressedBlock::from_raw(&stream_bytes, ZSTD_LEVEL)?;
        let envelope = build_envelope(0, &compressed, &[])?;
        let envelope_offset = HEADER_LEN + block.block_id * BLOCK_SIZE;
        self.dest_file.write_all_at(&envelope, envelope_offset)?;

        let len = u16::try_from(compressed.stripped.len())
            .map_err(|_| TileTreeError::CorruptFile("self payload exceeds u16"))?;
        let block_idx = usize::try_from(block.block_id)
            .map_err(|_| TileTreeError::InvalidBounds("block id exceeds usize"))?;
        self.len_self[block_idx] = len;

        Ok(())
    }
}
