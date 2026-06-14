pub mod xyz;

pub use xyz::{WEB_MERCATOR_MAX_LAT, XyzTile, XyzTileError, xyz_tile_bounds, xyz_tiles_for_bounds};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}
