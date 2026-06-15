use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::bounds::XYZBounds;
use super::error::TileTreeError;
use super::format::{payload_offset, write_header, write_index_entry};
use super::index::TileTreeIndexEntry;
use super::metadata::{TileKind, TileTreeMetadata};
use super::slot_index::SlotIndex;
use super::writer::TileTreeFile;

pub struct TileTreeFileBuilder {
    path: PathBuf,
    tile_kind: Option<TileKind>,
    bounds: Option<XYZBounds>,
}

impl TileTreeFileBuilder {
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            path,
            tile_kind: None,
            bounds: None,
        }
    }

    pub fn tile_kind(mut self, tile_kind: TileKind) -> Self {
        self.tile_kind = Some(tile_kind);
        self
    }

    pub fn bounds(mut self, bounds: XYZBounds) -> Self {
        self.bounds = Some(bounds);
        self
    }

    pub fn finish_header(self) -> Result<TileTreeFile, TileTreeError> {
        let tile_kind = self
            .tile_kind
            .ok_or(TileTreeError::MissingBuilderField("tile_kind"))?;
        let bounds = self
            .bounds
            .ok_or(TileTreeError::MissingBuilderField("bounds"))?;
        let metadata = TileTreeMetadata::new(tile_kind, bounds);
        let index = SlotIndex::new(bounds)?;
        let total_index_entries = index.total_entries();
        let tmp_path = temp_path_for(&self.path);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;

        write_header(&mut file, metadata)?;
        for _ in 0..total_index_entries {
            write_index_entry(&mut file, TileTreeIndexEntry::EMPTY)?;
        }
        file.seek(SeekFrom::Start(payload_offset(total_index_entries)))?;

        let entries = vec![
            TileTreeIndexEntry::EMPTY;
            usize::try_from(total_index_entries)
                .map_err(|_| TileTreeError::InvalidBounds("index is too large"))?
        ];

        Ok(TileTreeFile {
            path: self.path,
            tmp_path,
            file,
            metadata,
            index,
            entries,
        })
    }
}

pub(super) fn temp_path_for(path: &Path) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("tile-tree"));
    file_name.push(".tmp");
    path.with_file_name(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_path_appends_tmp_to_whole_file_name() {
        assert_eq!(
            temp_path_for(Path::new("terrain.tengri-dem")),
            PathBuf::from("terrain.tengri-dem.tmp")
        );
    }
}
