//! Per-block raw-tile cache used when the source can't supply intermediate
//! tiles (TIF, etc.). Each block written by the DFS orchestrator may spill its
//! 4096-or-fewer raws to a single file in [`std::env::temp_dir`]; once the
//! parent block reduces, all 4 children are loaded back into memory and the
//! child files are deleted.
//!
//! Layout: one file per block at
//! `temp_dir/tengri-tree-export-<pid>-<rand>/<zoom>/<block_id>.raw`.
//!
//! Loaded blocks are returned as `Arc<Vec<u8>>` plus an in-RAM offset table.
//! Workers slice into the shared bytes and call
//! [`TileTreeExportAdapter::read_raw_cache`] on the sub-slice when reducing a
//! parent tile, so we never deserialise tiles that aren't immediately needed
//! and we never require [`TileTreeExportAdapter::SourceTile: Clone`].

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::tree::error::TileTreeError;

use super::adapter::TileTreeExportAdapter;

pub(super) struct RawCache {
    root_path: PathBuf,
    blocks: HashMap<u64, BlockEntry>,
}

struct BlockEntry {
    file_path: PathBuf,
    /// Byte offsets per tile into the `bytes` vector; `len = tile_count + 1`,
    /// `offsets[i+1] - offsets[i]` is the i-th tile's serialised length.
    offsets: Vec<u32>,
}

/// Bytes for one cached block of raw riles, plus per-slot offsets. Cheap to
/// clone via `Arc` so workers can share without re-reading.
#[derive(Clone)]
pub(super) struct LoadedBlock {
    /// RAW tile payloads written on disk as a contiguous block of bytes.
    pub(super) bytes: Arc<Vec<u8>>,
    /// Byte offsets per tile into the `bytes` vector;
    pub(super) offsets: Arc<Vec<u32>>,
}

impl LoadedBlock {
    /// Returns a byte slice for the tile at the given slot.
    pub(super) fn slot_bytes(&self, slot: u32) -> Result<&[u8], TileTreeError> {
        let idx = slot as usize;
        if idx + 1 >= self.offsets.len() {
            return Err(TileTreeError::CorruptFile("raw cache slot out of bounds"));
        }

        let start = self.offsets[idx] as usize;
        let end = self.offsets[idx + 1] as usize;
        if end > self.bytes.len() || start > end {
            return Err(TileTreeError::CorruptFile("raw cache offsets corrupted"));
        }

        Ok(&self.bytes[start..end])
    }
}

impl RawCache {
    pub(super) fn new(destination: &Path) -> Result<Self, TileTreeError> {
        // Create a unique directory name for the raw cache.
        let output_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("tile-tree");
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| TileTreeError::CorruptFile("system time is before unix epoch"))?
            .as_nanos();
        let root_path = std::env::temp_dir().join(format!(
            "tengri-tree-export-{}-{created_at}-{output_name}",
            std::process::id()
        ));
        fs::create_dir_all(&root_path)?;

        Ok(Self {
            root_path,
            blocks: HashMap::new(),
        })
    }

    /// Write `tiles` (in slot order) to the block's cache file, recording an
    /// offset table so [`Self::load_and_drop`] can hand callers per-slot
    /// byte slices.
    pub(super) fn put<A: TileTreeExportAdapter>(
        &mut self,
        adapter: &A,
        zoom: u8,
        block_id: u64,
        tiles: &[A::SourceTile],
    ) -> Result<(), TileTreeError> {
        let path = self.path(zoom, block_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = File::create(&path)?;

        let mut writer = BufWriter::new(file);
        let mut offsets = Vec::with_capacity(tiles.len() + 1);
        let mut cursor: u32 = 0;
        offsets.push(cursor);
        for tile in tiles {
            let mut counted = ByteCountingWriter::new(&mut writer);
            adapter.write_raw_cache(&mut counted, tile)?;
            let written = u32::try_from(counted.count())
                .map_err(|_| TileTreeError::CorruptFile("raw cache tile too large"))?;
            cursor = cursor
                .checked_add(written)
                .ok_or(TileTreeError::CorruptFile("raw cache offset overflow"))?;
            offsets.push(cursor);
        }

        writer.flush()?;
        self.blocks.insert(
            block_id,
            BlockEntry {
                file_path: path,
                offsets,
            },
        );
        Ok(())
    }

    /// Read the block file back into memory, delete the on-disk file, and
    /// return shared bytes + per-slot offsets. Workers call
    /// [`LoadedBlock::slot_bytes`] to get a tile's bytes and feed them to
    /// [`TileTreeExportAdapter::read_raw_cache`].
    pub(super) fn load_and_drop(&mut self, block_id: u64) -> Result<LoadedBlock, TileTreeError> {
        let entry = self
            .blocks
            .remove(&block_id)
            .ok_or(TileTreeError::CorruptFile(
                "raw cache load_and_drop on missing block",
            ))?;
        let mut bytes = Vec::new();
        {
            let mut file = File::open(&entry.file_path)?;
            file.read_to_end(&mut bytes)?;
        }
        let _ = fs::remove_file(&entry.file_path);
        Ok(LoadedBlock {
            bytes: Arc::new(bytes),
            offsets: Arc::new(entry.offsets),
        })
    }

    /// Returns the path to the cached block file by its zoom and block ID.
    fn path(&self, zoom: u8, block_id: u64) -> PathBuf {
        self.root_path
            .join(zoom.to_string())
            .join(format!("{block_id}.raw"))
    }
}

impl Drop for RawCache {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root_path);
    }
}

/// Wraps an inner writer to count bytes written without forcing a flush of
/// the buffered writer (which would happen on `stream_position`).
struct ByteCountingWriter<'a, W: Write> {
    inner: &'a mut W,
    count: u64,
}

impl<'a, W: Write> ByteCountingWriter<'a, W> {
    fn new(inner: &'a mut W) -> Self {
        Self { inner, count: 0 }
    }

    fn count(&self) -> u64 {
        self.count
    }
}

impl<W: Write> Write for ByteCountingWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.count += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;
    use crate::geo::XyzTile;
    use crate::tree::{CachedChild, TileKind, TileTreeError, TileTreeExportAdapter, XYZBounds};

    struct FakeAdapter;

    impl TileTreeExportAdapter for FakeAdapter {
        type SourceTile = Vec<u8>;
        type Reader = ();

        fn tile_kind(&self) -> TileKind {
            TileKind::Dem
        }

        fn bounds(&self) -> XYZBounds {
            XYZBounds::new(0, 0, 0, 0, 0).unwrap()
        }

        fn open_reader(&self) -> Result<Self::Reader, TileTreeError> {
            Ok(())
        }

        fn read_source_tile(
            &self,
            _reader: &mut Self::Reader,
            _tile: XyzTile,
        ) -> Result<Self::SourceTile, TileTreeError> {
            unimplemented!("raw-cache unit tests don't exercise the source path")
        }

        fn encode_payload(&self, tile: &Self::SourceTile) -> Result<Vec<u8>, TileTreeError> {
            Ok(tile.clone())
        }

        fn write_raw_cache(
            &self,
            writer: &mut dyn Write,
            tile: &Self::SourceTile,
        ) -> Result<(), TileTreeError> {
            let len = tile.len() as u32;
            writer.write_all(&len.to_le_bytes())?;
            writer.write_all(tile)?;
            Ok(())
        }

        fn read_raw_cache(&self, reader: &mut dyn Read) -> Result<Self::SourceTile, TileTreeError> {
            let mut len_bytes = [0u8; 4];
            reader.read_exact(&mut len_bytes)?;
            let len = u32::from_le_bytes(len_bytes) as usize;
            let mut bytes = vec![0u8; len];
            reader.read_exact(&mut bytes)?;
            Ok(bytes)
        }

        fn reduce_children_to_tile(
            &self,
            _tile: XyzTile,
            _children: &[CachedChild<Self::SourceTile>],
        ) -> Result<Self::SourceTile, TileTreeError> {
            unimplemented!()
        }
    }

    #[test]
    fn spill_then_load_yields_byte_equal_tiles() {
        let dest = std::env::temp_dir().join("tengri-raw-cache-test.tengri-dem");
        let mut cache = RawCache::new(&dest).unwrap();
        let adapter = FakeAdapter;
        let tiles: Vec<Vec<u8>> = (0..8u8)
            .map(|i| {
                (0..((i as usize) * 11 + 1))
                    .map(|j| (j as u8) ^ i)
                    .collect()
            })
            .collect();
        cache.put(&adapter, 5, 42, &tiles).unwrap();

        let loaded = cache.load_and_drop(42).unwrap();
        for (slot, expected) in tiles.iter().enumerate() {
            let mut slice = loaded.slot_bytes(slot as u32).unwrap();
            let actual = adapter.read_raw_cache(&mut slice).unwrap();
            assert_eq!(&actual, expected);
        }
    }
}
