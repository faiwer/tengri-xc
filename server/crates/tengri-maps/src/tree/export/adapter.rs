use std::io::{Read, Write};

use crate::geo::XyzTile;
use crate::tree::{TileKind, TileTreeError, XYZBounds};

pub struct TileTreeExportReport {
    pub zoom: u8,
    pub tiles_written: usize,
}

pub struct CachedChild<T> {
    pub tile: XyzTile,
    pub raw: T,
}

pub trait TileTreeExportAdapter: Send + Sync + 'static {
    type SourceTile: Send + 'static;
    type Reader: Send + 'static;

    fn tile_kind(&self) -> TileKind;
    /// The bounds of the source tiles. Not every sources covers the whole world.
    fn bounds(&self) -> XYZBounds;
    /// Opens a reader for the source. Each worker thread opens its own reader.
    fn open_reader(&self) -> Result<Self::Reader, TileTreeError>;
    /// Reads the source raw tile when possible. Not every source supports it.
    fn try_read_source_tile(
        &self,
        reader: &mut Self::Reader,
        tile: XyzTile,
    ) -> Result<Option<Self::SourceTile>, TileTreeError>;
    /// Prepares the payload of the tile for writing to the tree file. Must be compressed.
    fn encode_payload(&self, tile: &Self::SourceTile) -> Result<Vec<u8>, TileTreeError>;
    /// Writes the !raw! tile to the cache file.
    fn write_raw_cache(
        &self,
        writer: &mut dyn Write,
        tile: &Self::SourceTile,
    ) -> Result<(), TileTreeError>;
    /// Reads the raw tile from the cache file.
    fn read_raw_cache(&self, reader: &mut dyn Read) -> Result<Self::SourceTile, TileTreeError>;
    /// Builds the parent tile from the children.
    fn reduce_children_to_tile(
        &self,
        tile: XyzTile,
        children: &[CachedChild<Self::SourceTile>],
    ) -> Result<Self::SourceTile, TileTreeError>;
}
