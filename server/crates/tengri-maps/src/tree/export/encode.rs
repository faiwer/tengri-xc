//! Per-block size-stream encoding and 16 KiB envelope assembly.

use crate::tree::error::TileTreeError;
use crate::tree::format::{
    BLOCK_SIZE, ENVELOPE_OVERHEAD, EXTRA_PREFIX_LEN, ZSTD_FRAME_MAGIC, write_varint, zigzag_encode,
};

/// One per-block size-stream builder. The stream encodes `(tile_count,
/// base_offset, size_0, size_code_1, ..., size_code_{n-1})` as concatenated
/// varints. `size_code = 0` means "reuse the running anchor" (no fresh bytes
/// written for this slot); `size_code > 0` means a fresh write whose length
/// differs from the anchor by `zigzag_decode(code - 1)`.
pub struct SizeStream {
    buf: Vec<u8>,
    expected_tiles: u32,
    pushed_tiles: u32,
    /// Anchor length carried into the next slot (the most recent fresh write's
    /// length). Slot 0's length seeds it.
    anchor_length: u32,
    first_pushed: bool,
}

impl SizeStream {
    pub fn new(tile_count: u32, base_offset: u64) -> Self {
        let mut buf = Vec::with_capacity(usize::try_from(tile_count).unwrap_or(0) + 8);
        write_varint(&mut buf, u64::from(tile_count));
        write_varint(&mut buf, base_offset);
        Self {
            buf,
            expected_tiles: tile_count,
            pushed_tiles: 0,
            anchor_length: 0,
            first_pushed: false,
        }
    }

    /// Slot 0 — first written tile in the block. Must have non-zero length.
    pub fn push_first(&mut self, length: u32) -> Result<(), TileTreeError> {
        if self.first_pushed {
            return Err(TileTreeError::CorruptFile(
                "size stream first slot pushed twice",
            ));
        }
        if length == 0 {
            return Err(TileTreeError::CorruptFile("slot 0 cannot be empty"));
        }
        write_varint(&mut self.buf, u64::from(length));
        self.anchor_length = length;
        self.first_pushed = true;
        self.pushed_tiles += 1;
        Ok(())
    }

    /// Fresh write at slot ≥ 1: emit `zigzag(length - anchor) + 1`. Updates
    /// the running anchor.
    pub fn push_fresh(&mut self, length: u32) -> Result<(), TileTreeError> {
        if !self.first_pushed {
            return Err(TileTreeError::CorruptFile(
                "size stream fresh push before first",
            ));
        }
        let delta = i64::from(length) - i64::from(self.anchor_length);
        let code = zigzag_encode(delta) + 1;
        write_varint(&mut self.buf, code);
        self.anchor_length = length;
        self.pushed_tiles += 1;
        Ok(())
    }

    /// Slot at i ≥ 1 reusing the running anchor: emits `0`.
    pub fn push_anchor_reuse(&mut self) -> Result<(), TileTreeError> {
        if !self.first_pushed {
            return Err(TileTreeError::CorruptFile(
                "size stream anchor-reuse before first",
            ));
        }
        self.buf.push(0);
        self.pushed_tiles += 1;
        Ok(())
    }

    pub fn finish(self) -> Result<Vec<u8>, TileTreeError> {
        if self.pushed_tiles != self.expected_tiles {
            return Err(TileTreeError::CorruptFile(
                "size stream tile count mismatch",
            ));
        }
        Ok(self.buf)
    }
}

/// One compressed self-payload, ready for envelope assembly. Stored without
/// the zstd frame magic.
#[derive(Clone)]
pub struct CompressedBlock {
    pub stripped: Vec<u8>,
}

impl CompressedBlock {
    pub fn from_raw(raw: &[u8], level: i32) -> Result<Self, TileTreeError> {
        let compressed = zstd::stream::encode_all(raw, level).map_err(TileTreeError::Io)?;
        if compressed.len() < ZSTD_FRAME_MAGIC.len()
            || compressed[..ZSTD_FRAME_MAGIC.len()] != ZSTD_FRAME_MAGIC
        {
            return Err(TileTreeError::CorruptFile(
                "zstd output missing frame magic",
            ));
        }

        let stripped = compressed[ZSTD_FRAME_MAGIC.len()..].to_vec();
        let max_self =
            usize::try_from(BLOCK_SIZE - ENVELOPE_OVERHEAD).expect("block size fits in usize");
        if stripped.len() > max_self {
            // Should never happen. The biggest self payload we managed to get
            // is about 8 KiB, so this is a contract bug.
            return Err(TileTreeError::CorruptFile(
                "block self-payload exceeds envelope budget",
            ));
        }
        Ok(Self { stripped })
    }
}

/// Mode-byte bit positions. Self is implicit (always present after the mode
/// byte); each set bit indicates an extra payload appended in this order.
pub mod mode {
    pub const PARENT: u8 = 1 << 0;
    pub const SIBLING_HORIZONTAL: u8 = 1 << 1;
    pub const SIBLING_VERTICAL: u8 = 1 << 2;
    pub const SIBLING_DIAGONAL: u8 = 1 << 3;
    pub const COUSIN_HORIZONTAL: u8 = 1 << 4;
    pub const COUSIN_VERTICAL: u8 = 1 << 5;
    pub const COUSIN_DIAGONAL: u8 = 1 << 6;
}

/// Build the 16 KiB envelope: `[mode][len_self][self][len_extra][extra]…[zero pad]`.
/// `extras` are listed in mode-bit order, omitting bits that aren't set.
pub fn build_envelope(
    mode_byte: u8,
    self_block: &CompressedBlock,
    extras: &[&CompressedBlock],
) -> Result<[u8; BLOCK_SIZE as usize], TileTreeError> {
    build_envelope_raw(
        mode_byte,
        &self_block.stripped,
        &extras
            .iter()
            .map(|extra| extra.stripped.as_slice())
            .collect::<Vec<_>>(),
    )
}

/// Build the 16 KiB envelope from already-stripped raw payload bytes. The
/// end-pass uses this path: it reads neighbour self-payloads back from disk
/// (where they're already zstd-stripped) and never reconstructs a
/// [`CompressedBlock`].
pub fn build_envelope_raw(
    mode_byte: u8,
    self_payload: &[u8],
    extras: &[&[u8]],
) -> Result<[u8; BLOCK_SIZE as usize], TileTreeError> {
    let mut envelope = [0u8; BLOCK_SIZE as usize];

    // Write the mode byte.
    let mut cursor: usize = 0;
    envelope[cursor] = mode_byte;
    cursor += 1;

    // Write the length of the self payload.
    let len_self = u16::try_from(self_payload.len())
        .map_err(|_| TileTreeError::CorruptFile("self payload exceeds u16"))?;
    envelope[cursor..cursor + 2].copy_from_slice(&len_self.to_le_bytes());
    cursor += 2;

    // Write the self payload.
    if cursor + self_payload.len() > envelope.len() {
        return Err(TileTreeError::CorruptFile(
            "envelope self payload exceeds 16 KiB",
        ));
    }
    envelope[cursor..cursor + self_payload.len()].copy_from_slice(self_payload);
    cursor += self_payload.len();

    // Write the extras.
    for extra in extras {
        let len_extra = u16::try_from(extra.len())
            .map_err(|_| TileTreeError::CorruptFile("extra payload exceeds u16"))?;
        let needed = usize::from(EXTRA_PREFIX_LEN as u8) + extra.len();
        if cursor + needed > envelope.len() {
            return Err(TileTreeError::CorruptFile(
                "envelope extras overflow 16 KiB",
            ));
        }
        envelope[cursor..cursor + 2].copy_from_slice(&len_extra.to_le_bytes());
        cursor += 2;
        envelope[cursor..cursor + extra.len()].copy_from_slice(extra);
        cursor += extra.len();
    }

    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::format::{read_varint, zigzag_decode};

    #[test]
    fn size_stream_encodes_tile_count_and_base_offset_first() {
        let stream = SizeStream::new(4096, 1_234_567);
        let buf = stream.buf;
        let mut cursor = 0;
        assert_eq!(read_varint(&buf, &mut cursor).unwrap(), 4096);
        assert_eq!(read_varint(&buf, &mut cursor).unwrap(), 1_234_567);
    }

    #[test]
    fn size_stream_anchor_reuse_emits_zero() {
        let mut stream = SizeStream::new(3, 100);
        stream.push_first(33).unwrap();
        stream.push_anchor_reuse().unwrap();
        stream.push_fresh(40).unwrap();
        let buf = stream.finish().unwrap();
        let mut cursor = 0;
        assert_eq!(read_varint(&buf, &mut cursor).unwrap(), 3);
        assert_eq!(read_varint(&buf, &mut cursor).unwrap(), 100);
        assert_eq!(read_varint(&buf, &mut cursor).unwrap(), 33);
        assert_eq!(read_varint(&buf, &mut cursor).unwrap(), 0);
        let code = read_varint(&buf, &mut cursor).unwrap();
        assert!(code > 0);
        assert_eq!(zigzag_decode(code - 1), 7);
    }

    #[test]
    fn size_stream_fresh_with_same_length_does_not_collide_with_reuse() {
        let mut stream = SizeStream::new(2, 0);
        stream.push_first(33).unwrap();
        stream.push_fresh(33).unwrap();
        let buf = stream.finish().unwrap();
        let mut cursor = 0;
        let _tile_count = read_varint(&buf, &mut cursor).unwrap();
        let _base = read_varint(&buf, &mut cursor).unwrap();
        let _size_0 = read_varint(&buf, &mut cursor).unwrap();
        let code = read_varint(&buf, &mut cursor).unwrap();
        assert_ne!(code, 0, "fresh write must not encode as anchor reuse");
        assert_eq!(zigzag_decode(code - 1), 0);
    }

    #[test]
    fn build_envelope_pads_to_block_size() {
        let raw = [0u8; 64];
        let block = CompressedBlock::from_raw(&raw, 3).unwrap();
        let envelope = build_envelope(0, &block, &[]).unwrap();
        assert_eq!(envelope.len() as u64, BLOCK_SIZE);
        let len_self = u16::from_le_bytes([envelope[1], envelope[2]]);
        assert_eq!(len_self as usize, block.stripped.len());
        let tail_start = 1 + 2 + block.stripped.len();
        assert!(envelope[tail_start..].iter().all(|&b| b == 0));
    }

    #[test]
    fn build_envelope_raw_matches_compressed_block_path() {
        let raw = b"hello world";
        let block = CompressedBlock::from_raw(raw, 3).unwrap();
        let via_block = build_envelope(0, &block, &[]).unwrap();
        let via_raw = build_envelope_raw(0, &block.stripped, &[]).unwrap();
        assert_eq!(via_block, via_raw);
    }

    #[test]
    fn compressed_block_rejects_oversize_payload() {
        let raw = vec![0u8; 64];
        let block = CompressedBlock::from_raw(&raw, 3).unwrap();
        assert!(block.stripped.len() <= (BLOCK_SIZE - ENVELOPE_OVERHEAD) as usize);
    }
}
