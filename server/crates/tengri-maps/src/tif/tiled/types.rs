use crate::geo::Bounds;

use crate::tif::TifPixelMatrix;

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
    pub origin_lon: f64,
    pub origin_lat: f64,
    pub pixel_width_degrees: f64,
    pub pixel_height_degrees: f64,
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
