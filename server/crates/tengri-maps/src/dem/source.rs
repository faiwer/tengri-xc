use super::DemChunk;
use crate::geo::XyzTile;
use crate::tree::{TileTreeError, XYZBounds};

/// Backend that produces DEM tiles for the tree exporter to ingest.
///
/// One implementor per source format (PMTiles + WebP-Terrarium, tiled GeoTIFF,
/// …). The exporter calls [`open_reader`](Self::open_reader) once per worker
/// thread; readers are not shared across threads.
pub trait DemSource: Send + Sync {
    /// Tile-space rectangle the source covers, at the source's native zoom. The
    /// exporter writes leaves at this zoom and reduces upward.
    fn tile_bounds(&self) -> XYZBounds;

    fn open_reader(&self) -> Result<Box<dyn DemSourceReader>, TileTreeError>;

    /// `true` when the source can serve tiles at zoom levels above the leaf
    /// zoom directly (e.g. PMTiles ships pre-rendered overviews). The exporter
    /// then prefers the source's tile and only falls back to pyramid reduction
    /// on [`TileTreeError::MissingTile`]. `false` (the default) means leaves
    /// only — every parent is built by reducing children.
    fn reads_intermediate_tiles(&self) -> bool {
        false
    }
}

/// Reader handle owned by a single worker thread. Readers are stateful (file
/// cursor, decoder) and never shared.
pub trait DemSourceReader: Send {
    fn read(&mut self, tile: XyzTile) -> Result<DemChunk, TileTreeError>;
}

impl<T: DemSource + ?Sized> DemSource for Box<T> {
    fn tile_bounds(&self) -> XYZBounds {
        (**self).tile_bounds()
    }

    fn open_reader(&self) -> Result<Box<dyn DemSourceReader>, TileTreeError> {
        (**self).open_reader()
    }

    fn reads_intermediate_tiles(&self) -> bool {
        (**self).reads_intermediate_tiles()
    }
}
