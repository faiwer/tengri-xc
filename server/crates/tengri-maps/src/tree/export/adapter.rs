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
    /// Source coverage shape. Two states:
    ///   - `true`  — full pyramid: every tile at every zoom in `bounds()`
    ///               (e.g. PMTiles). The orchestrator source-directs at
    ///               every zoom and never spills the raw cache.
    ///   - `false` — leaves only: tiles exist only at `bounds().zoom`
    ///               (e.g. a single-resolution GeoTIFF). Parents are built
    ///               by reducing their children through the raw cache.
    /// There is no "some intermediate zooms" middle ground — the source
    /// must commit to one of these.
    fn supplies_all_zooms(&self) -> bool {
        false
    }
    /// Reads the raw tile from the source. The orchestrator only calls this
    /// when [`supplies_all_zooms`] or `tile.z == bounds().zoom` already
    /// guarantees the tile is present, so a miss here is a contract bug
    /// (returned as [`TileTreeError::MissingTile`]).
    fn read_source_tile(
        &self,
        reader: &mut Self::Reader,
        tile: XyzTile,
    ) -> Result<Self::SourceTile, TileTreeError>;
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
