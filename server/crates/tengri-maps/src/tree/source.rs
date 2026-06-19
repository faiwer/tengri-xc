use crate::geo::XyzTile;
use crate::tree::{TileTreeError, XYZBounds};

/// Per-format tile producer feeding the tree exporter. One implementor per
/// backing source (PMTiles + WebP-Terrarium DEM, PMTiles WebP imagery, tiled
/// GeoTIFF, …); the [`Tile`](Self::Tile) associated type is the per-tile shape
/// the source emits — `DemChunk` for elevation, [`crate::matrix::Raster`] for
/// imagery — chosen by the source, consumed downstream by the matching
/// [`crate::tree::TileTreeExportAdapter`].
///
/// The exporter calls [`open_reader`](Self::open_reader) once per worker
/// thread; the returned readers are stateful (decoder cursors, file handles)
/// and never shared.
pub trait TileSource: Send + Sync {
    /// Per-tile output shape. Different kinds emit different shapes (numeric
    /// grids vs decoded pixel rasters), but the source/reader contract is
    /// otherwise identical across kinds.
    type Tile: Send;

    /// Tile-space rectangle the source covers, at its native zoom. The
    /// exporter writes leaves at this zoom and reduces upward.
    fn tile_bounds(&self) -> XYZBounds;

    fn open_reader(
        &self,
    ) -> Result<Box<dyn TileSourceReader<Tile = Self::Tile>>, TileTreeError>;

    /// `true` when the source ships a dense pyramid — every tile at every
    /// zoom inside `tile_bounds()` is present and the source will serve
    /// it directly (PMTiles is the canonical case). The exporter then
    /// source-directs every block and skips the raw-tile cache entirely;
    /// reduce-from-children becomes unreachable. A source that returns
    /// `true` here MUST NOT raise `MissingTile` for any tile inside its
    /// bounds — that's a contract bug.
    ///
    /// `false` (default) means leaves only: every parent block is built
    /// by reducing its children through the raw cache.
    fn reads_intermediate_tiles(&self) -> bool {
        false
    }

    /// Maximum number of zoom levels this source can downsample below its
    /// native pixel pitch when serving a single leaf tile read. Default
    /// `u8::MAX` (no constraint); [`crate::tif::TifDemSource`] overrides to `1`
    /// because its per-tile read path can't materialise a region wider than two
    /// native tiles per side.
    fn max_leaf_downsample_steps(&self) -> u8 {
        u8::MAX
    }

    /// Codec the source can deliver via
    /// [`TileSourceReader::read_raw`]. `None` (the default) = no
    /// passthrough channel: the exporter must always go through
    /// [`TileSourceReader::read`] and encode itself.
    ///
    /// Exporters whose target codec doesn't match what's returned here
    /// never consult `read_raw`, so a non-matching codec is a no-op
    /// rather than a contract violation.
    fn raw_codec(&self) -> Option<PassthroughCodec> {
        None
    }
}

/// Reader handle owned by a single worker thread. Readers are stateful (file
/// cursor, decoder) and never shared.
pub trait TileSourceReader: Send {
    type Tile;

    fn read(&mut self, tile: XyzTile) -> Result<Self::Tile, TileTreeError>;

    /// Optional secondary channel: raw source bytes for `tile` *without*
    /// decoding to the matrix. Default `Ok(None)` — no passthrough. Only
    /// consulted by exporters whose target codec matches
    /// [`TileSource::raw_codec`]; the exporter is responsible for validating
    /// the bytes (e.g. dim peek) before writing them verbatim.
    fn read_raw(&mut self, _tile: XyzTile) -> Result<Option<Vec<u8>>, TileTreeError> {
        Ok(None)
    }
}

/// Source byte format the source can deliver via the passthrough channel.
/// Exporters compare this against their own target codec to decide whether the
/// fast-path is reachable; no match = matrix path only.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassthroughCodec {
    Webp,
    // Png, Jpeg, Avif — added when their decoders land
}

/// What the export pipeline moves per tile when the target supports a
/// verbatim-bytes fast-path. Either the decoded matrix (the normal path, which
/// the encoder will compress for the archive) or pre-baked target-codec bytes
/// that bypass the encoder.
///
/// Targets that *can't* passthrough (e.g. `.tengri-dem`) bind directly on the
/// matrix shape (`SourceTile = DemChunk`) and never wrap.
#[derive(Debug, Clone)]
pub enum TilePayload<M> {
    Matrix(M),
    Passthrough(Vec<u8>),
}

impl<T: TileSource + ?Sized> TileSource for Box<T> {
    type Tile = T::Tile;

    fn tile_bounds(&self) -> XYZBounds {
        (**self).tile_bounds()
    }

    fn open_reader(
        &self,
    ) -> Result<Box<dyn TileSourceReader<Tile = Self::Tile>>, TileTreeError> {
        (**self).open_reader()
    }

    fn reads_intermediate_tiles(&self) -> bool {
        (**self).reads_intermediate_tiles()
    }

    fn max_leaf_downsample_steps(&self) -> u8 {
        (**self).max_leaf_downsample_steps()
    }

    fn raw_codec(&self) -> Option<PassthroughCodec> {
        (**self).raw_codec()
    }
}
