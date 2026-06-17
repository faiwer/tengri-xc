pub mod mercator;
pub mod xyz;

pub use mercator::{
    WEB_MERCATOR_HALF_EQUATOR_M, WEB_MERCATOR_RADIUS_M, lat_to_mercator_y_m, lon_to_mercator_x_m,
    mercator_x_m_to_lon, mercator_y_m_to_lat,
};
pub use xyz::{WEB_MERCATOR_MAX_LAT, XyzTile, XyzTileError, xyz_tile_bounds, xyz_tiles_for_bounds};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}
