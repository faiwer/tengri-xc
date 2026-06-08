use std::error::Error;
use std::f64::consts::PI;
use std::fmt;

use crate::geo::Bounds;

pub const WEB_MERCATOR_MAX_LAT: f64 = 85.051_128_779_806_6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XyzTile {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XyzTileError {
    InvalidBounds,
    UnsupportedZoom(u8),
}

pub fn xyz_tile_bounds(zoom: u8, lng: u32, lat: u32) -> Result<Bounds, XyzTileError> {
    let tiles_per_side = tiles_per_side_by_zoom(zoom)?;
    if lng >= tiles_per_side || lat >= tiles_per_side {
        return Err(XyzTileError::InvalidBounds);
    }

    let scale = f64::from(tiles_per_side);
    Ok(Bounds {
        min_lat: latitude_for_tile_edge(lat + 1, scale),
        min_lon: (f64::from(lng) / scale) * 360.0 - 180.0,
        max_lat: latitude_for_tile_edge(lat, scale),
        max_lon: (f64::from(lng + 1) / scale) * 360.0 - 180.0,
    })
}

pub fn xyz_tiles_for_bounds(bounds: Bounds, z: u8) -> Result<Vec<XyzTile>, XyzTileError> {
    validate_bounds(bounds)?;
    let tiles_per_side = tiles_per_side_by_zoom(z)?;
    let scale = f64::from(tiles_per_side);

    let min_x = lower_tile_index(lon_to_tile_x(bounds.min_lon, scale), tiles_per_side);
    let max_x = upper_tile_index(lon_to_tile_x(bounds.max_lon, scale), tiles_per_side);
    let north = bounds.max_lat.min(WEB_MERCATOR_MAX_LAT);
    let south = bounds.min_lat.max(-WEB_MERCATOR_MAX_LAT);
    if south >= north {
        return Err(XyzTileError::InvalidBounds);
    }
    let min_y = lower_tile_index(lat_to_tile_y(north, scale), tiles_per_side);
    let max_y = upper_tile_index(lat_to_tile_y(south, scale), tiles_per_side);

    let mut tiles = Vec::new();
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            tiles.push(XyzTile { z, x, y });
        }
    }
    Ok(tiles)
}

impl fmt::Display for XyzTileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XyzTileError::InvalidBounds => write!(formatter, "invalid XYZ tile bounds"),
            XyzTileError::UnsupportedZoom(z) => {
                write!(formatter, "unsupported XYZ tile zoom {z}")
            }
        }
    }
}

impl Error for XyzTileError {}

/// Returns how many XYZ tiles exist along one edge of the world at that zoom
fn tiles_per_side_by_zoom(zoom: u8) -> Result<u32, XyzTileError> {
    1u32.checked_shl(u32::from(zoom))
        .ok_or(XyzTileError::UnsupportedZoom(zoom))
}

fn validate_bounds(bounds: Bounds) -> Result<(), XyzTileError> {
    if !bounds.min_lat.is_finite()
        || !bounds.min_lon.is_finite()
        || !bounds.max_lat.is_finite()
        || !bounds.max_lon.is_finite()
        || bounds.min_lat >= bounds.max_lat
        || bounds.min_lon >= bounds.max_lon
        || bounds.min_lon < -180.0
        || bounds.max_lon > 180.0
        || bounds.min_lat < -90.0
        || bounds.max_lat > 90.0
    {
        return Err(XyzTileError::InvalidBounds);
    }
    Ok(())
}

fn lower_tile_index(value: f64, tiles_per_side: u32) -> u32 {
    value.floor().clamp(0.0, f64::from(tiles_per_side - 1)) as u32
}

fn upper_tile_index(value: f64, tiles_per_side: u32) -> u32 {
    (value.ceil() - 1.0).clamp(0.0, f64::from(tiles_per_side - 1)) as u32
}

fn lon_to_tile_x(lon: f64, scale: f64) -> f64 {
    ((lon + 180.0) / 360.0) * scale
}

fn lat_to_tile_y(lat: f64, scale: f64) -> f64 {
    let lat_rad = lat.to_radians();
    ((1.0 - lat_rad.tan().asinh() / PI) / 2.0) * scale
}

fn latitude_for_tile_edge(y: u32, scale: f64) -> f64 {
    let mercator_y = PI * (1.0 - 2.0 * f64::from(y) / scale);
    mercator_y.sinh().atan().to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zxy_to_bounds_uses_web_mercator_edges() {
        let bounds = xyz_tile_bounds(1, 0, 0).unwrap();

        assert_eq!(bounds.min_lon, -180.0);
        assert_eq!(bounds.max_lon, 0.0);
        assert!(bounds.min_lat.abs() < 1e-12);
        assert!((bounds.max_lat - WEB_MERCATOR_MAX_LAT).abs() < 1e-12);
    }

    #[test]
    fn bounds_to_tiles_returns_intersecting_tiles() {
        let tiles = xyz_tiles_for_bounds(
            Bounds {
                min_lat: -1.0,
                min_lon: -1.0,
                max_lat: 1.0,
                max_lon: 1.0,
            },
            2,
        )
        .unwrap();

        assert_eq!(
            tiles,
            vec![
                XyzTile { z: 2, x: 1, y: 1 },
                XyzTile { z: 2, x: 1, y: 2 },
                XyzTile { z: 2, x: 2, y: 1 },
                XyzTile { z: 2, x: 2, y: 2 },
            ]
        );
    }
}
