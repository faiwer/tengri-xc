use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use super::builder::TileTreeFileBuilder;
use super::error::TileTreeError;
use super::format::{index_offset, write_index_entry, write_magic};
use super::index::TileTreeIndexEntry;
use super::metadata::TileTreeMetadata;
use super::slot_index::SlotIndex;

pub struct TileTreeFile {
    pub(super) path: PathBuf,
    pub(super) tmp_path: PathBuf,
    pub(super) file: File,
    pub(super) metadata: TileTreeMetadata,
    pub(super) index: SlotIndex,
    pub(super) entries: Vec<TileTreeIndexEntry>,
    /// Maps `hash(payload) -> entry` so identical payloads share a single
    /// on-disk copy. SipHash-64; collision probability at 1M tiles is ~1e-8,
    /// which we trust for this build.
    pub(super) payloads: HashMap<u64, TileTreeIndexEntry>,
}

impl TileTreeFile {
    pub fn create(path: impl AsRef<Path>) -> TileTreeFileBuilder {
        TileTreeFileBuilder::new(path.as_ref().to_owned())
    }

    pub fn add(
        &mut self,
        z: u8,
        x: u16,
        y: u16,
        payload: &[u8],
    ) -> Result<&mut Self, TileTreeError> {
        let slot = self.index.slot(z, x, y)?;
        if !self.entries[slot].is_empty() {
            return Err(TileTreeError::DuplicateTile { z, x, y });
        }

        let hash = hash_payload(payload);
        let entry = if let Some(&existing) = self.payloads.get(&hash) {
            existing
        } else {
            let entry: TileTreeIndexEntry = self.write_payload(payload)?;
            self.payloads.insert(hash, entry);
            entry
        };

        self.entries[slot] = entry;
        self.file.seek(SeekFrom::Start(index_offset(slot)))?;
        write_index_entry(&mut self.file, entry)?;
        self.file.seek(SeekFrom::End(0))?;
        Ok(self)
    }

    fn write_payload(&mut self, payload: &[u8]) -> Result<TileTreeIndexEntry, TileTreeError> {
        self.file.seek(SeekFrom::End(0))?;
        let offset = self.file.stream_position()?;
        self.file.write_all(payload)?;
        let end = self.file.stream_position()?;
        let length = end - offset;
        let length = u32::try_from(length).map_err(|_| TileTreeError::TileTooLarge(length))?;
        Ok(TileTreeIndexEntry { offset, length })
    }

    pub fn finish(mut self) -> Result<(), TileTreeError> {
        self.ensure_complete()?;
        self.file.seek(SeekFrom::End(0))?;
        write_magic(&mut self.file)?;
        self.file.flush()?;
        drop(self.file);
        fs::rename(&self.tmp_path, &self.path)?;
        Ok(())
    }

    /// Ensures that all tiles are present in the tree.
    fn ensure_complete(&self) -> Result<(), TileTreeError> {
        for z in (0..=self.metadata.bounds.zoom).rev() {
            for tile in self.metadata.bounds.tiles_at(z)? {
                let x = tile.x as u16;
                let y = tile.y as u16;
                let slot = self.index.slot(z, x, y)?;
                if self.entries[slot].is_empty() {
                    return Err(TileTreeError::MissingTile { z, x, y });
                }
            }
        }
        Ok(())
    }
}

fn hash_payload(payload: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    payload.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::builder::temp_path_for;
    use super::*;
    use crate::tree::{TileKind, TileTreeReader, XYZBounds};

    #[test]
    fn finish_rejects_missing_entries() {
        let path = test_path("tengri-missing");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(temp_path_for(&path));
        let bounds = XYZBounds::new(0, 0, 0, 0, 0).unwrap();
        let tree = TileTreeFile::create(&path)
            .tile_kind(TileKind::Dem)
            .bounds(bounds)
            .finish_header()
            .unwrap();

        assert!(matches!(
            tree.finish(),
            Err(TileTreeError::MissingTile {
                z: 0,
                x: 0,
                y: 0
            })
        ));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(temp_path_for(&path));
    }

    #[test]
    fn identical_payloads_share_a_single_on_disk_copy() {
        let path = test_path("tengri-dedup");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(temp_path_for(&path));
        let bounds = XYZBounds::new(1, 0, 0, 1, 1).unwrap();
        let shared = b"identical-tile-payload";
        let unique = b"unique-tile-payload";

        let mut tree = TileTreeFile::create(&path)
            .tile_kind(TileKind::Dem)
            .bounds(bounds)
            .finish_header()
            .unwrap();
        tree.add(0, 0, 0, shared).unwrap();
        tree.add(1, 0, 0, shared).unwrap();
        tree.add(1, 1, 0, shared).unwrap();
        tree.add(1, 0, 1, unique).unwrap();
        tree.add(1, 1, 1, shared).unwrap();

        let shared_entry = tree.entries[tree.index.slot(0, 0, 0).unwrap()];
        for (z, x, y) in [(1, 0, 0), (1, 1, 0), (1, 1, 1)] {
            assert_eq!(
                tree.entries[tree.index.slot(z, x, y).unwrap()],
                shared_entry,
                "tile ({z},{x},{y}) should reuse the shared payload's entry",
            );
        }
        let unique_entry = tree.entries[tree.index.slot(1, 0, 1).unwrap()];
        assert_ne!(unique_entry, shared_entry);

        tree.finish().unwrap();

        let mut reader = TileTreeReader::open(&path).unwrap();
        for (z, x, y) in [(0, 0, 0), (1, 0, 0), (1, 1, 0), (1, 1, 1)] {
            assert_eq!(reader.read(z, x, y).unwrap(), shared.to_vec());
        }
        assert_eq!(reader.read(1, 0, 1).unwrap(), unique.to_vec());

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(temp_path_for(&path));
    }

    #[test]
    fn finished_tree_roundtrips_tiles_by_coordinate() {
        let path = test_path("tengri-roundtrip");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(temp_path_for(&path));
        let bounds = XYZBounds::new(0, 0, 0, 0, 0).unwrap();
        let payload = b"terrain";

        let mut tree = TileTreeFile::create(&path)
            .tile_kind(TileKind::Dem)
            .bounds(bounds)
            .finish_header()
            .unwrap();
        tree.add(0, 0, 0, payload).unwrap();
        tree.finish().unwrap();

        let mut reader = TileTreeReader::open(&path).unwrap();
        assert_eq!(reader.metadata().bounds, bounds);
        assert_eq!(reader.metadata().tile_kind, TileKind::Dem);
        assert_eq!(reader.read(0, 0, 0).unwrap(), payload.to_vec());

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(temp_path_for(&path));
    }

    fn test_path(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/output/tree-tests");
        fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{name}-{}.tengri-dem", std::process::id()))
    }
}
