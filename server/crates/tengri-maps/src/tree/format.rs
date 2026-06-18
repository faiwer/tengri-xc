//! Wire format for the compact `.tengri-dem` archive.
//!
//! Single fixed-size header at offset 0, then `block_count` envelopes of
//! exactly `BLOCK_SIZE` bytes each, then a packed tile-data section, then a
//! footer magic. Block envelopes hold zstd-compressed size-streams that
//! reconstruct each slot's `(offset, length)` via prefix-sum, plus optional
//! parent / sibling / cousin payloads packed into the leftover headroom.

use std::io::{Read, Write};

use super::bounds::XYZBounds;
use super::error::TileTreeError;
use super::metadata::{TileKind, TileTreeMetadata};

pub const MAGIC: [u8; 4] = *b"TTRC";
pub const VERSION: u8 = 1;

/// Fixed header length. See [`write_header`] for the field layout.
pub const HEADER_LEN: u64 = 66;

/// Per-block envelope size on disk. Every block is written into exactly this
/// many bytes regardless of compressed payload size; the leftover is filled
/// with optional neighbour payloads + zero-pad.
///
/// 16 KiB so a viewer pulls a whole envelope in a single round trip on a
/// warm HTTPS connection: it matches one TLS 1.3 record (RFC 8446 §5.1) and
/// one HTTP/2 DATA frame, so neither layer fragments the response.
pub const BLOCK_SIZE: u64 = 16_384;

/// Tiles per block edge (square). 64 is picked so 4096 slots' size-stream
/// reliably fits the 16 KiB envelope after zstd.
pub const BLOCK_W: u8 = 64;
pub const BLOCK_H: u8 = 64;

/// Self-payload envelope overhead: `[mode_byte: u8][len_self: u16]`. Phase 2's
/// compressed self-payload must fit in `BLOCK_SIZE - ENVELOPE_OVERHEAD`.
pub const ENVELOPE_OVERHEAD: u64 = 1 + 2;

/// Per-extra prefix inside the envelope: `[len_extra: u16]`.
pub const EXTRA_PREFIX_LEN: u64 = 2;

/// Zstd frame magic. Stripped from each stored payload; the reader prepends
/// it back before decoding.
pub const ZSTD_FRAME_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// Decoded form of the on-disk header. Several fields are validated during
/// decode and not re-used by the runtime; they're still exposed for tooling.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct CompactHeader {
    pub tile_kind: TileKind,
    pub min_zoom: u8,
    pub bounds: XYZBounds,
    pub block_w: u8,
    pub block_h: u8,
    pub tile_data_len: u64,
    pub payload_hash: [u8; 32],
}

impl CompactHeader {
    pub fn metadata(&self) -> TileTreeMetadata {
        TileTreeMetadata::new(self.tile_kind, self.bounds)
    }
}

pub fn write_header(
    writer: &mut impl Write,
    metadata: TileTreeMetadata,
    min_zoom: u8,
    tile_data_len: u64,
    payload_hash: [u8; 32],
) -> Result<(), TileTreeError> {
    let bounds = metadata.bounds;
    if min_zoom > bounds.zoom {
        return Err(TileTreeError::InvalidBounds("min_zoom exceeds max_zoom"));
    }
    writer.write_all(&MAGIC)?;
    writer.write_all(&[VERSION])?;
    writer.write_all(&[metadata.tile_kind.to_u8()])?;
    writer.write_all(&[min_zoom])?;
    writer.write_all(&[bounds.zoom])?;
    writer.write_all(&bounds.min_x.to_le_bytes())?;
    writer.write_all(&bounds.min_y.to_le_bytes())?;
    writer.write_all(&bounds.max_x.to_le_bytes())?;
    writer.write_all(&bounds.max_y.to_le_bytes())?;
    writer.write_all(&[BLOCK_W])?;
    writer.write_all(&[BLOCK_H])?;
    writer.write_all(&tile_data_len.to_le_bytes())?;
    writer.write_all(&payload_hash)?;
    // Reserved for future use.
    writer.write_all(&[0u8; 8])?;
    Ok(())
}

pub fn read_header(reader: &mut impl Read) -> Result<CompactHeader, TileTreeError> {
    let mut magic = [0; 4];
    reader.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(TileTreeError::CorruptFile("missing tile tree header magic"));
    }

    let version = read_u8(reader)?;
    if version != VERSION {
        return Err(TileTreeError::CorruptFile("unsupported tile tree version"));
    }

    let tile_kind = TileKind::from_u8(read_u8(reader)?)
        .ok_or(TileTreeError::CorruptFile("unsupported tree tile kind"))?;
    let min_zoom = read_u8(reader)?;
    let max_zoom = read_u8(reader)?;
    if min_zoom > max_zoom {
        return Err(TileTreeError::CorruptFile("min_zoom exceeds max_zoom"));
    }
    let bounds = XYZBounds::new(
        max_zoom,
        read_u16(reader)?,
        read_u16(reader)?,
        read_u16(reader)?,
        read_u16(reader)?,
    )?;
    let block_w = read_u8(reader)?;
    let block_h = read_u8(reader)?;
    if block_w != BLOCK_W || block_h != BLOCK_H {
        return Err(TileTreeError::CorruptFile("unsupported block dimensions"));
    }
    let tile_data_len = read_u64(reader)?;
    let mut payload_hash = [0u8; 32];
    reader.read_exact(&mut payload_hash)?;
    let mut reserved = [0u8; 8];
    reader.read_exact(&mut reserved)?;

    Ok(CompactHeader {
        tile_kind,
        min_zoom,
        bounds,
        block_w,
        block_h,
        tile_data_len,
        payload_hash,
    })
}

/// LEB128-style unsigned varint writer. 1 bit per byte is used to signal that
/// more bytes follow.
pub fn write_varint(buf: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        buf.push((value as u8) | 0x80);
        value >>= 7;
    }
    buf.push(value as u8);
}

/// LEB128-style unsigned varint reader. Advances `cursor` past the consumed
/// bytes; returns `CorruptFile` on truncation or overflow.
pub fn read_varint(slice: &[u8], cursor: &mut usize) -> Result<u64, TileTreeError> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = *slice
            .get(*cursor)
            .ok_or(TileTreeError::CorruptFile("varint truncated"))?;
        *cursor += 1;
        value |= u64::from(byte & 0x7F) << shift;
        if byte < 0x80 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 64 {
            return Err(TileTreeError::CorruptFile("varint overflow"));
        }
    }
}

/// Zigzag encoding is a variable-length encoding of signed integers that
/// efficiently represents both positive and negative values using only unsigned
/// integers. It's commonly used in lossless data compression algorithms like
/// Zstandard (zstd) to reduce the size of the data being compressed.
///
/// The basic idea is to represent a signed integer as an unsigned integer by
/// encoding the difference between the current and previous value. Positive
/// values are encoded as even numbers, and negative values are encoded as odd
/// numbers.
pub fn zigzag_encode(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

pub fn zigzag_decode(n: u64) -> i64 {
    ((n >> 1) as i64) ^ -((n & 1) as i64)
}

fn read_u8(reader: &mut impl Read) -> Result<u8, TileTreeError> {
    let mut bytes = [0; 1];
    reader.read_exact(&mut bytes)?;
    Ok(bytes[0])
}

fn read_u16(reader: &mut impl Read) -> Result<u16, TileTreeError> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Result<u64, TileTreeError> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        for &value in &[0u64, 1, 127, 128, 1024, u64::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, value);
            let mut cursor = 0;
            assert_eq!(read_varint(&buf, &mut cursor).unwrap(), value);
            assert_eq!(cursor, buf.len(), "varint not fully consumed for {value}");
        }
    }

    #[test]
    fn zigzag_roundtrip() {
        for &value in &[0i64, 1, -1, 4096, -4096, i64::MAX, i64::MIN] {
            assert_eq!(zigzag_decode(zigzag_encode(value)), value);
        }
    }

    #[test]
    fn read_varint_rejects_truncated() {
        let buf = [0x80u8, 0x80, 0x80];
        let mut cursor = 0;
        assert!(read_varint(&buf, &mut cursor).is_err());
    }

    #[test]
    fn read_varint_rejects_overflow() {
        // 10 continuation bytes would shift past 63.
        let buf = [0xFFu8; 11];
        let mut cursor = 0;
        assert!(read_varint(&buf, &mut cursor).is_err());
    }

    #[test]
    fn header_roundtrip() {
        let bounds = XYZBounds::new(11, 100, 200, 300, 400).unwrap();
        let metadata = TileTreeMetadata::new(TileKind::Dem, bounds);
        let mut buf = Vec::new();
        let hash = [7u8; 32];
        write_header(&mut buf, metadata, 4, 12_345_678, hash).unwrap();
        assert_eq!(buf.len() as u64, HEADER_LEN);

        let mut slice = buf.as_slice();
        let header = read_header(&mut slice).unwrap();
        assert_eq!(header.tile_kind, TileKind::Dem);
        assert_eq!(header.bounds, bounds);
        assert_eq!(header.min_zoom, 4);
        assert_eq!(header.block_w, BLOCK_W);
        assert_eq!(header.block_h, BLOCK_H);
        assert_eq!(header.tile_data_len, 12_345_678);
        assert_eq!(header.payload_hash, hash);
    }

    #[test]
    fn write_header_rejects_min_zoom_above_max() {
        let bounds = XYZBounds::new(4, 0, 0, 0, 0).unwrap();
        let metadata = TileTreeMetadata::new(TileKind::Dem, bounds);
        let mut buf = Vec::new();
        let err = write_header(&mut buf, metadata, 5, 0, [0u8; 32]).unwrap_err();
        assert!(matches!(err, TileTreeError::InvalidBounds(_)));
    }
}
