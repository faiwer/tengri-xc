use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::geo::XyzTile;
use crate::tree::TileTreeError;

use super::adapter::TileTreeExportAdapter;

pub(super) struct RawTileCache {
    root: PathBuf,
}

impl RawTileCache {
    pub(super) fn new(destination: &Path) -> Result<Self, TileTreeError> {
        let output_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("tile-tree");
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| TileTreeError::CorruptFile("system time is before unix epoch"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tengri-tree-export-{}-{created_at}-{output_name}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub(super) fn write<A: TileTreeExportAdapter>(
        &self,
        adapter: &A,
        tile: XyzTile,
        raw: &A::SourceTile,
    ) -> Result<(), TileTreeError> {
        let path = self.path(tile);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(path)?;
        adapter.write_raw_cache(&mut file, raw)
    }

    pub(super) fn consume<A: TileTreeExportAdapter>(
        &self,
        adapter: &A,
        tile: XyzTile,
    ) -> Result<Option<A::SourceTile>, TileTreeError> {
        let path = self.path(tile);
        let mut file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(TileTreeError::Io(error)),
        };
        let raw = adapter.read_raw_cache(&mut file)?;
        drop(file);
        // The algo is written in a way we never need to read the same tile twice.
        fs::remove_file(path)?;
        Ok(Some(raw))
    }

    pub(super) fn path(&self, tile: XyzTile) -> PathBuf {
        self.root
            .join(tile.z.to_string())
            .join(tile.x.to_string())
            .join(format!("{}.raw", tile.y))
    }
}

impl Drop for RawTileCache {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
