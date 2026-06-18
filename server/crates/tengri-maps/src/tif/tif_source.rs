use std::path::{Path, PathBuf};

use crate::dem::constants::MAX_DEM_TILE_SIDE;
use crate::dem::{DemSource, DemSourceReader};
use crate::geo::{Bounds, WEB_MERCATOR_HALF_EQUATOR_M, xyz_tiles_for_bounds};
use crate::tif::TiledTifReader;
use crate::tif::tif_dem_source_reader::TifDemSourceReader;
use crate::tree::{TileTreeError, XYZBounds};

use super::tiled::TifProjection;

pub struct TifDemSource {
    path: PathBuf,
    tile_bounds: XYZBounds,
    read_bounds: Bounds,
}

impl TifDemSource {
    pub fn open(path: impl AsRef<Path>, bounds: Option<Bounds>) -> Result<Self, TileTreeError> {
        let reader = TiledTifReader::open(&path)?;
        let info = reader.info();
        let zoom = source_backed_leaf_zoom(info.projection, info.pixel_width);
        let read_bounds = bounds.unwrap_or(info.bounds);
        let tiles = xyz_tiles_for_bounds(read_bounds, zoom)?;
        let tile_bounds = XYZBounds::from_tiles(zoom, &tiles)?;

        Ok(Self {
            path: path.as_ref().to_owned(),
            tile_bounds,
            read_bounds,
        })
    }
}

impl DemSource for TifDemSource {
    fn tile_bounds(&self) -> XYZBounds {
        self.tile_bounds
    }

    fn open_reader(&self) -> Result<Box<dyn DemSourceReader>, TileTreeError> {
        Ok(Box::new(TifDemSourceReader {
            reader: TiledTifReader::open(&self.path)?,
            read_bounds: self.read_bounds,
        }))
    }

    /// `read_region` rejects native pixel rectangles wider than `2 ×
    /// MAX_DEM_TILE_SIDE` (the `validate_exact_region_dimensions` guard in
    /// `tif/tiled/downscale.rs`), so a leaf read can pull at most one zoom
    /// level below the source's native pixel pitch.
    fn max_leaf_downsample_steps(&self) -> u8 {
        1
    }
}

/// Largest XYZ zoom whose tile grid fits inside the source raster at one DEM
/// tile per XYZ tile. The denominator is the world's extent in the source's
/// native units: 360° for [`TifProjection::Wgs84`] (the world spans 360° lon at
/// the equator), or `2 · WEB_MERCATOR_HALF_EQUATOR_M` (~40 075 km) for
/// [`TifProjection::WebMercator`].
fn source_backed_leaf_zoom(projection: TifProjection, pixel_width: f64) -> u8 {
    let world_extent = match projection {
        TifProjection::Wgs84 => 360.0,
        TifProjection::WebMercator => 2.0 * WEB_MERCATOR_HALF_EQUATOR_M,
    };
    let source_tiles_across =
        ((world_extent / pixel_width) / f64::from(MAX_DEM_TILE_SIDE)).floor() as u32;
    if source_tiles_across == 0 {
        return 0;
    }

    u32::BITS as u8 - 1 - source_tiles_across.leading_zeros() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16384 px wide WGS84 source at 360° / 16384 px = ~0.022°/px hosts
    /// 16384 / 256 = 64 = 2^6 DEM tiles across — z=6 is the leaf.
    #[test]
    fn wgs84_source_picks_zoom_from_pixel_width() {
        assert_eq!(source_backed_leaf_zoom(TifProjection::Wgs84, 360.0 / 16384.0), 6);
    }

    /// A 32768 px Web-Mercator world raster has 32768 / 256 = 128 = 2^7
    /// DEM tiles across — z=7 is the leaf.
    #[test]
    fn web_mercator_source_picks_zoom_from_metric_pixel_width() {
        let pixel = (2.0 * WEB_MERCATOR_HALF_EQUATOR_M) / 32768.0;
        assert_eq!(source_backed_leaf_zoom(TifProjection::WebMercator, pixel), 7);
    }

    /// A coarser Mercator raster (16384 px world ≈ 2.4 km/px) hosts only
    /// 64 = 2^6 DEM tiles, mirroring the equivalent WGS84 case at the
    /// same DEM-tile count.
    #[test]
    fn web_mercator_z6_world_round_trips_to_zoom_six() {
        let pixel = (2.0 * WEB_MERCATOR_HALF_EQUATOR_M) / 16384.0;
        assert_eq!(source_backed_leaf_zoom(TifProjection::WebMercator, pixel), 6);
    }

    /// Sub-DEM-tile rasters can't host any XYZ tile and degrade to z=0.
    #[test]
    fn rasters_too_small_for_one_tile_clamp_to_zero() {
        assert_eq!(source_backed_leaf_zoom(TifProjection::Wgs84, 4.0), 0);
        assert_eq!(
            source_backed_leaf_zoom(TifProjection::WebMercator, WEB_MERCATOR_HALF_EQUATOR_M),
            0,
        );
    }
}
