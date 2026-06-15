use std::fs::{self, File};
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

        self.file.seek(SeekFrom::End(0))?;
        let offset = self.file.stream_position()?;
        self.file.write_all(payload)?;
        let end = self.file.stream_position()?;
        let length = end - offset;
        let length = u32::try_from(length).map_err(|_| TileTreeError::TileTooLarge(length))?;
        let entry = TileTreeIndexEntry { offset, length };

        self.entries[slot] = entry;
        self.file.seek(SeekFrom::Start(index_offset(slot)))?;
        write_index_entry(&mut self.file, entry)?;
        self.file.seek(SeekFrom::End(0))?;
        Ok(self)
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
