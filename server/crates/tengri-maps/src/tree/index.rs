#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileTreeIndexEntry {
    /// Offset of the tile payload in the file in bytes. Starts from the start
    /// of the file.
    pub offset: u64,
    /// Length of the tile payload in bytes.
    pub length: u32,
}

impl TileTreeIndexEntry {
    pub const EMPTY: Self = Self {
        offset: 0,
        length: 0,
    };

    pub fn is_empty(self) -> bool {
        self.offset == 0 && self.length == 0
    }
}
