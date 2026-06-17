use super::error::TileTreeError;
use super::format::{read_varint, zigzag_decode};

#[derive(Debug, Clone, Copy)]
pub(super) struct SizeAnswer {
    /// Offset of the tile payload in the tile-data section of a .tengri-map
    /// file.
    pub(super) offset: u64,
    /// Length of the tile payload in bytes.
    pub(super) length: u32,
}

/// Walks the size stream and returns the offset and length of the requested
/// slot (tile payload in a .tengri-dem filmap
pub(super) fn walk_size_stream(
    payload: &[u8],
    slot_in_block: u32,
) -> Result<SizeAnswer, TileTreeError> {
    let mut walker = SizeStreamWalker::start(payload)?;
    let mut answer = walker.first()?;
    if slot_in_block == 0 {
        return Ok(answer);
    }
    for _ in 0..slot_in_block {
        answer = walker.next_slot(payload)?;
    }
    Ok(answer)
}

pub(super) struct SizeStreamWalker {
    /// Cursor into the payload section of the envelope.
    cursor_into_payload: usize,
    /// Cursor into the destination tile-data section in a .tengri-dem fimap
    file_cursor: u64,
    /// On-disk start offset of the last tile we yielded. Re-emitted as-is on a
    /// `code == 0` repeat slot.
    anchor_offset: u64,
    /// Byte length of the last tile we yielded. Reused on a `code == 0` repeat
    /// slot, and used as the base for delta-decoding the next slot's length.
    anchor_length: u32,
    /// Index of the next slot to yield.
    next_slot_index: u32,
    /// Number of tiles in the size stream.
    tile_count: u32,
    /// Whether the first slot has been yielded.
    first_yielded: bool,
}

/// Walks the index-block's .sizes varint[]. A size-stream is a sequence of
/// varints that encode the length of each tile. The first varint is the number
/// of tiles in the size stream. The second varint is the base offset of the
/// size stream. The third varint is the length of the first tile. The remaining
/// varints are the lengths of the subsequent tiles.
impl SizeStreamWalker {
    pub(super) fn start(payload: &[u8]) -> Result<Self, TileTreeError> {
        let mut cursor = 0;
        let tile_count = read_varint(payload, &mut cursor)?;
        let base_offset = read_varint(payload, &mut cursor)?;
        let size_0 = read_varint(payload, &mut cursor)?;
        let tile_count = u32::try_from(tile_count)
            .map_err(|_| TileTreeError::CorruptFile("tile_count exceeds u32"))?;
        let size_0 =
            u32::try_from(size_0).map_err(|_| TileTreeError::CorruptFile("size_0 exceeds u32"))?;
        if size_0 == 0 {
            return Err(TileTreeError::CorruptFile("size_0 is zero"));
        }
        Ok(Self {
            cursor_into_payload: cursor,
            file_cursor: base_offset,
            anchor_offset: base_offset,
            anchor_length: size_0,
            next_slot_index: 0,
            tile_count,
            first_yielded: false,
        })
    }

    /// Slot 0's answer. Must be called before [`Self::next_slot`].
    fn first(&mut self) -> Result<SizeAnswer, TileTreeError> {
        if self.first_yielded {
            return Err(TileTreeError::CorruptFile(
                "size stream first slot yielded twice",
            ));
        }

        self.first_yielded = true;
        self.next_slot_index = 1;
        let answer = SizeAnswer {
            offset: self.anchor_offset,
            length: self.anchor_length,
        };
        // Update the file cursor to the end of the first tile.
        self.file_cursor = self
            .file_cursor
            .checked_add(u64::from(self.anchor_length))
            .ok_or(TileTreeError::CorruptFile("file cursor overflow"))?;
        Ok(answer)
    }

    pub(super) fn next_slot(&mut self, payload: &[u8]) -> Result<SizeAnswer, TileTreeError> {
        if !self.first_yielded {
            return self.first();
        }

        if self.next_slot_index >= self.tile_count {
            return Err(TileTreeError::CorruptFile(
                "size stream walked past tile_count",
            ));
        }

        let code = read_varint(payload, &mut self.cursor_into_payload)?;
        self.next_slot_index += 1;
        if code == 0 {
            // Anchor reuse: the next slot has the same length as the previous slot.
            return Ok(SizeAnswer {
                offset: self.anchor_offset,
                length: self.anchor_length,
            });
        }

        let delta = zigzag_decode(code - 1);
        let length_signed = i64::from(self.anchor_length)
            .checked_add(delta)
            .ok_or(TileTreeError::CorruptFile("size stream length overflow"))?;
        if length_signed <= 0 {
            return Err(TileTreeError::CorruptFile(
                "size stream produced non-positive length",
            ));
        }
        let length = u32::try_from(length_signed)
            .map_err(|_| TileTreeError::CorruptFile("length exceeds u32"))?;
        let offset = self.file_cursor;
        self.anchor_offset = offset;
        self.anchor_length = length;
        // Update the file cursor to the end of the current tile.
        self.file_cursor = self
            .file_cursor
            .checked_add(u64::from(length))
            .ok_or(TileTreeError::CorruptFile("file cursor overflow"))?;
        Ok(SizeAnswer { offset, length })
    }
}
