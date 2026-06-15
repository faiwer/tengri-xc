use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::error::TileTreeError;
use super::format::{MAGIC, read_header, read_index_entry};
use super::index::TileTreeIndexEntry;
use super::metadata::TileTreeMetadata;
use super::slot_index::SlotIndex;

pub struct TileTreeReader {
    file: File,
    metadata: TileTreeMetadata,
    index: SlotIndex,
    entries: Vec<TileTreeIndexEntry>,
}

impl TileTreeReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TileTreeError> {
        let mut file = File::open(path)?;
        let metadata = read_header(&mut file)?;
        let index = SlotIndex::new(metadata.bounds)?;
        let entry_count = index.total_entries();
        let mut entries = Vec::with_capacity(
            usize::try_from(entry_count)
                .map_err(|_| TileTreeError::InvalidBounds("index is too large"))?,
        );
        for _ in 0..entry_count {
            entries.push(read_index_entry(&mut file)?);
        }
        if entries.iter().any(|entry| entry.is_empty()) {
            return Err(TileTreeError::CorruptFile(
                "index contains an empty tile entry",
            ));
        }

        validate_footer(&mut file)?;

        Ok(Self {
            file,
            metadata,
            index,
            entries,
        })
    }

    pub fn metadata(&self) -> TileTreeMetadata {
        self.metadata
    }

    pub fn read(&mut self, z: u8, x: u16, y: u16) -> Result<Vec<u8>, TileTreeError> {
        let slot = self.index.slot(z, x, y)?;
        let entry = self.entries[slot];
        if entry.is_empty() {
            return Err(TileTreeError::MissingTile { z, x, y });
        }

        self.file.seek(SeekFrom::Start(entry.offset))?;
        let mut payload = vec![0; entry.length as usize];
        self.file.read_exact(&mut payload)?;
        Ok(payload)
    }
}

fn validate_footer(file: &mut File) -> Result<(), TileTreeError> {
    let len = file.metadata()?.len();
    if len < u64::try_from(MAGIC.len()).unwrap() {
        return Err(TileTreeError::CorruptFile(
            "file is too short for footer magic",
        ));
    }

    file.seek(SeekFrom::End(-(MAGIC.len() as i64)))?;
    let mut magic = [0; 4];
    file.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(TileTreeError::CorruptFile("missing tile tree footer magic"));
    }
    Ok(())
}
