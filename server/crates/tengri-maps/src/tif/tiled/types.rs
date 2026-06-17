use crate::geo::Bounds;

use crate::tif::types::TifPixelMatrix;

use super::projection::TifProjection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PixelRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct TiledTifInfo {
    pub width: u32,
    pub height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub tiles_across: u32,
    pub tiles_down: u32,
    pub projection: TifProjection,
    /// X coordinate of the upper-left pixel's left edge, in this TIFF's
    /// projection-native units (degrees lon for [`TifProjection::Wgs84`],
    /// metres for [`TifProjection::WebMercator`]).
    pub origin_x: f64,
    /// Y coordinate of the upper-left pixel's top edge, same unit as
    /// [`TiledTifInfo::origin_x`].
    pub origin_y: f64,
    /// Pixel width in projection-native units.
    pub pixel_width: f64,
    /// Pixel height in projection-native units (always positive; rows grow
    /// southward, so a row's top y equals `origin_y - row · pixel_height`).
    pub pixel_height: f64,
    /// Geographic extent of the raster in lat/lon degrees, computed once at
    /// open. Stored separately so callers don't have to know the source
    /// projection to ask "what's covered?".
    pub bounds: Bounds,
}

#[derive(Debug)]
pub struct TiledTifChunk {
    pub tile_x: u32,
    pub tile_y: u32,
    pub width: u32,
    pub height: u32,
    pub pixels: TifPixelMatrix,
}
