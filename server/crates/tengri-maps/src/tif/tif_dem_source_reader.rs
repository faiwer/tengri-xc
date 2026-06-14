use crate::dem::{DemChunk, DemSourceReader};
use crate::geo::{Bounds, XyzTile, xyz_tile_bounds};
use crate::tif::TiledTifReader;
use crate::tree::TileTreeError;

pub(super) struct TifDemSourceReader {
    pub(super) reader: TiledTifReader,
    pub(super) read_bounds: Bounds,
}

impl DemSourceReader for TifDemSourceReader {
    fn read(&mut self, tile: XyzTile) -> Result<DemChunk, TileTreeError> {
        let bounds = xyz_tile_bounds(tile.z, tile.x, tile.y)?;
        let bounds = intersect_bounds(bounds, self.read_bounds).ok_or(
            TileTreeError::CorruptFile("XYZ tile does not intersect source bounds"),
        )?;
        Ok(self.reader.read_region(bounds)?)
    }
}

fn intersect_bounds(a: Bounds, b: Bounds) -> Option<Bounds> {
    let bounds = Bounds {
        min_lat: a.min_lat.max(b.min_lat),
        min_lon: a.min_lon.max(b.min_lon),
        max_lat: a.max_lat.min(b.max_lat),
        max_lon: a.max_lon.min(b.max_lon),
    };

    (bounds.min_lat < bounds.max_lat && bounds.min_lon < bounds.max_lon).then_some(bounds)
}
