use std::path::{Path, PathBuf};

use crate::dem::constants::MAX_DEM_TILE_SIDE;
use crate::dem::{DemSource, DemSourceReader};
use crate::geo::{Bounds, xyz_tiles_for_bounds};
use crate::tif::TiledTifReader;
use crate::tif::tif_dem_source_reader::TifDemSourceReader;
use crate::tree::{TileTreeError, WebMercatorTileBounds};

pub struct TifDemSource {
    path: PathBuf,
    tile_bounds: WebMercatorTileBounds,
    read_bounds: Bounds,
}

impl TifDemSource {
    pub fn open(path: impl AsRef<Path>, bounds: Option<Bounds>) -> Result<Self, TileTreeError> {
        let reader = TiledTifReader::open(&path)?;
        let info = reader.info();
        let zoom = source_backed_leaf_zoom(info.pixel_width_degrees);
        let read_bounds = bounds.unwrap_or(info.bounds);
        let tiles = xyz_tiles_for_bounds(read_bounds, zoom)?;
        let tile_bounds = WebMercatorTileBounds::from_tiles(zoom, &tiles)?;

        Ok(Self {
            path: path.as_ref().to_owned(),
            tile_bounds,
            read_bounds,
        })
    }
}

impl DemSource for TifDemSource {
    fn tile_bounds(&self) -> WebMercatorTileBounds {
        self.tile_bounds
    }

    fn open_reader(&self) -> Result<Box<dyn DemSourceReader>, TileTreeError> {
        Ok(Box::new(TifDemSourceReader {
            reader: TiledTifReader::open(&self.path)?,
            read_bounds: self.read_bounds,
        }))
    }
}

fn source_backed_leaf_zoom(pixel_width_degrees: f64) -> u8 {
    let source_tiles_across =
        ((360.0 / pixel_width_degrees) / f64::from(MAX_DEM_TILE_SIDE)).floor() as u32;
    if source_tiles_across == 0 {
        return 0;
    }

    u32::BITS as u8 - 1 - source_tiles_across.leading_zeros() as u8
}
