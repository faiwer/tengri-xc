//! Reader for the compact `.tengri-dem` format.
//!
//! The header at offset 0 carries the bounds and the tile-data section length;
//! everything else is computable from those two facts. Every tile lookup is
//! `(z, x, y) → block_id (arithmetic) → 16 KiB envelope (one read) → decompress
//! block payload → walk size-stream to the requested slot → `(offset, length)`
//! → read tile bytes`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::Path;

use super::blocks::BlockGrid;
use super::bounds::XYZBounds;
use super::error::TileTreeError;
use super::format::{
    BLOCK_SIZE, ENVELOPE_OVERHEAD, EXTRA_PREFIX_LEN, HEADER_LEN, MAGIC, ZSTD_FRAME_MAGIC,
    read_header,
};
use super::metadata::TileTreeMetadata;
use super::size_stream::{SizeStreamWalker, walk_size_stream};
use super::slot_index::SlotIndex;

pub struct TileTreeReader {
    file: File,
    metadata: TileTreeMetadata,
    grid: BlockGrid,
    tile_data_off: u64,
    tile_data_len: u64,
}

impl TileTreeReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TileTreeError> {
        let mut file = File::open(path)?;
        let header = read_header(&mut file)?;
        let grid = BlockGrid::new(header.bounds, header.min_zoom)?;
        let tile_data_off = HEADER_LEN
            .checked_add(
                grid.total_blocks()
                    .checked_mul(BLOCK_SIZE)
                    .ok_or(TileTreeError::CorruptFile("block region overflow"))?,
            )
            .ok_or(TileTreeError::CorruptFile("block region overflow"))?;
        let tile_data_len = header.tile_data_len;

        validate_footer(&mut file, tile_data_off + tile_data_len)?;
        let metadata = header.metadata();
        Ok(Self {
            file,
            metadata,
            grid,
            tile_data_off,
            tile_data_len,
        })
    }

    pub fn metadata(&self) -> TileTreeMetadata {
        self.metadata
    }

    pub fn bounds(&self) -> XYZBounds {
        self.metadata.bounds
    }

    pub fn read(&mut self, z: u8, x: u16, y: u16) -> Result<Vec<u8>, TileTreeError> {
        let location = self.grid.block_for(z, x, y)?;
        let envelope = self.fetch_envelope(location.block_id)?;
        let payload = decompress_self(&envelope)?;
        let target = walk_size_stream(&payload, location.slot_in_block)?;
        if target.offset < self.tile_data_off
            || target
                .offset
                .checked_add(u64::from(target.length))
                .filter(|end| *end <= self.tile_data_off + self.tile_data_len)
                .is_none()
        {
            return Err(TileTreeError::CorruptFile(
                "tile (offset, length) outside tile-data section",
            ));
        }
        let mut bytes = vec![0u8; target.length as usize];
        self.file.read_exact_at(&mut bytes, target.offset)?;
        Ok(bytes)
    }
    fn fetch_envelope(&self, block_id: u64) -> Result<[u8; BLOCK_SIZE as usize], TileTreeError> {
        let mut envelope = [0u8; BLOCK_SIZE as usize];
        let offset = HEADER_LEN + block_id * BLOCK_SIZE;
        self.file.read_exact_at(&mut envelope, offset)?;
        Ok(envelope)
    }
}

fn validate_footer(file: &mut File, tile_data_end: u64) -> Result<(), TileTreeError> {
    let len = file.metadata()?.len();
    let footer_len = MAGIC.len() as u64;
    if len < tile_data_end + footer_len {
        return Err(TileTreeError::CorruptFile(
            "file is too short for footer magic",
        ));
    }

    file.seek(SeekFrom::Start(tile_data_end))?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(TileTreeError::CorruptFile("missing tile tree footer magic"));
    }
    Ok(())
}

/// Bounds-check the envelope's mode/len fields and decompress the self payload.
/// Walks past the extras' length prefixes too, so a corrupt `len_extra` (one
/// that would run off the envelope) surfaces here even though we never decode
/// the extras.
fn decompress_self(envelope: &[u8; BLOCK_SIZE as usize]) -> Result<Vec<u8>, TileTreeError> {
    if envelope.len() < ENVELOPE_OVERHEAD as usize {
        return Err(TileTreeError::CorruptFile("envelope too small"));
    }

    let mode_byte = envelope[0];
    let len_self = u16::from_le_bytes([envelope[1], envelope[2]]) as usize;
    let self_end = (ENVELOPE_OVERHEAD as usize) + len_self;
    if self_end > envelope.len() {
        return Err(TileTreeError::CorruptFile(
            "envelope self-payload exceeds 16 KiB",
        ));
    }

    let mut cursor = self_end;
    for bit in 0..7 {
        let mask = 1u8 << bit;
        if mode_byte & mask == 0 {
            // bit-th extra is not set, so skip it
            continue;
        }

        if cursor + (EXTRA_PREFIX_LEN as usize) > envelope.len() {
            return Err(TileTreeError::CorruptFile(
                "envelope extra prefix exceeds 16 KiB",
            ));
        }

        let len_extra = u16::from_le_bytes([envelope[cursor], envelope[cursor + 1]]) as usize;
        cursor += EXTRA_PREFIX_LEN as usize;
        if cursor + len_extra > envelope.len() {
            return Err(TileTreeError::CorruptFile(
                "envelope extra payload exceeds 16 KiB",
            ));
        }
        cursor += len_extra;
    }

    if mode_byte & 0x80 != 0 {
        return Err(TileTreeError::CorruptFile("envelope reserved mode bit set"));
    }

    let stripped = &envelope[ENVELOPE_OVERHEAD as usize..self_end];
    let mut framed = Vec::with_capacity(ZSTD_FRAME_MAGIC.len() + stripped.len());
    framed.extend_from_slice(&ZSTD_FRAME_MAGIC);
    framed.extend_from_slice(stripped);
    let payload = zstd::stream::decode_all(framed.as_slice()).map_err(TileTreeError::Io)?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::format::{BLOCK_SIZE, ENVELOPE_OVERHEAD};

    fn envelope_with_self_len(len_self: u16) -> [u8; BLOCK_SIZE as usize] {
        let mut envelope = [0u8; BLOCK_SIZE as usize];
        envelope[0] = 0;
        envelope[1..3].copy_from_slice(&len_self.to_le_bytes());
        envelope
    }

    #[test]
    fn decompress_self_rejects_oversize_len_self() {
        let too_big = (BLOCK_SIZE as u16) - 2; // overflows by 1.
        let envelope = envelope_with_self_len(too_big);
        let err = decompress_self(&envelope).unwrap_err();
        assert!(matches!(err, TileTreeError::CorruptFile(_)));
    }

    #[test]
    fn decompress_self_rejects_extra_overflow() {
        // mode_byte sets bit 0 (parent), len_self = 0 (no self payload),
        // len_extra = BLOCK_SIZE - ENVELOPE_OVERHEAD (overflows by exactly
        // the prefix length).
        let mut envelope = [0u8; BLOCK_SIZE as usize];
        envelope[0] = 1; // parent bit
        envelope[1..3].copy_from_slice(&0u16.to_le_bytes());
        let cursor = ENVELOPE_OVERHEAD as usize;
        let len_extra = ((BLOCK_SIZE - ENVELOPE_OVERHEAD) as u16).saturating_sub(1);
        envelope[cursor..cursor + 2].copy_from_slice(&len_extra.to_le_bytes());
        // Don't set actual extra bytes — the bounds check should still
        // permit this (extra fits exactly). Force it to overflow by adding 1.
        let len_extra_overflow =
            ((BLOCK_SIZE - ENVELOPE_OVERHEAD - EXTRA_PREFIX_LEN) as u16).saturating_add(1);
        envelope[cursor..cursor + 2].copy_from_slice(&len_extra_overflow.to_le_bytes());
        let err = decompress_self(&envelope).unwrap_err();
        assert!(matches!(err, TileTreeError::CorruptFile(_)));
    }

    #[test]
    fn decompress_self_rejects_reserved_mode_bit() {
        let mut envelope = envelope_with_self_len(0);
        envelope[0] = 0x80;
        let err = decompress_self(&envelope).unwrap_err();
        assert!(matches!(err, TileTreeError::CorruptFile(_)));
    }
}
