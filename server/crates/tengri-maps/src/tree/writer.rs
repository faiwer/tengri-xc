use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use super::builder::TileTreeFileBuilder;
use super::error::TileTreeError;
use super::format::{index_offset, write_index_entry, write_magic};
use super::index::TileTreeIndexEntry;
use super::metadata::TileTreeMetadata;

pub struct TileTreeFile {
    pub(super) path: PathBuf,
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
        let slot = self.metadata.bounds.slot(z, x, y)?;
    }

    pub fn read(&mut self, z: u8, lng: u16, lat: u16) -> Result<Vec<u8>, TileTreeError> {
    }

    pub fn finish(mut self) -> Result<(), TileTreeError> {

    pub(crate) fn entry(
    ) -> Result<TileTreeIndexEntry, TileTreeError> {
    }

    fn read_entry(
    ) -> Result<Vec<u8>, TileTreeError> {
    }

    fn ensure_complete(&self) -> Result<(), TileTreeError> {
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
