use crate::geo::Bounds;

use crate::tif::error::TiffReadError;

use super::types::{PixelRegion, TiledTifInfo};

pub(super) fn pixel_region_for_bounds(
    bounds: Bounds,
    info: TiledTifInfo,
) -> Result<PixelRegion, TiffReadError> {
    if !bounds.min_lat.is_finite()
        || !bounds.min_lon.is_finite()
        || !bounds.max_lat.is_finite()
        || !bounds.max_lon.is_finite()
    {
        return Err(TiffReadError::InvalidBounds(
            "all coordinates must be finite",
        ));
    }
    if bounds.min_lat >= bounds.max_lat || bounds.min_lon >= bounds.max_lon {
        return Err(TiffReadError::InvalidBounds(
            "minimum coordinates must be less than maximum coordinates",
        ));
    }

    let left = ((bounds.min_lon - info.origin_lon) / info.pixel_width_degrees).floor();
    let right = ((bounds.max_lon - info.origin_lon) / info.pixel_width_degrees).ceil();
    let top = ((info.origin_lat - bounds.max_lat) / info.pixel_height_degrees).floor();
    let bottom = ((info.origin_lat - bounds.min_lat) / info.pixel_height_degrees).ceil();

    if left < 0.0
        || top < 0.0
        || right > f64::from(info.width)
        || bottom > f64::from(info.height)
        || left >= right
        || top >= bottom
    {
        return Err(TiffReadError::RegionOutOfBounds);
    }

    Ok(PixelRegion {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}

pub(super) fn source_bounds(
    width: u32,
    height: u32,
    origin_lon: f64,
    origin_lat: f64,
    pixel_width_degrees: f64,
    pixel_height_degrees: f64,
) -> Bounds {
    Bounds {
        min_lat: origin_lat - f64::from(height) * pixel_height_degrees,
        min_lon: origin_lon,
        max_lat: origin_lat,
        max_lon: origin_lon + f64::from(width) * pixel_width_degrees,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_info() -> TiledTifInfo {
        TiledTifInfo {
            width: 8,
            height: 4,
            tile_width: 4,
            tile_height: 4,
            tiles_across: 2,
            tiles_down: 1,
            origin_lon: 0.0,
            origin_lat: 4.0,
            pixel_width_degrees: 1.0,
            pixel_height_degrees: 1.0,
            bounds: Bounds {
                min_lat: 0.0,
                min_lon: 0.0,
                max_lat: 4.0,
                max_lon: 8.0,
            },
        }
    }

    #[test]
    fn source_bounds_come_from_origin_scale_and_dimensions() {
        let bounds = source_bounds(3600, 3600, 10.0, 48.0, 1.0 / 3600.0, 1.0 / 3600.0);

        assert_close(bounds.min_lat, 47.0);
        assert_close(bounds.min_lon, 10.0);
        assert_close(bounds.max_lat, 48.0);
        assert_close(bounds.max_lon, 11.0);
    }

    #[test]
    fn bounds_round_outward_to_pixel_region() -> Result<(), TiffReadError> {
        let region = pixel_region_for_bounds(
            Bounds {
                min_lat: 0.25,
                min_lon: 1.25,
                max_lat: 2.75,
                max_lon: 5.25,
            },
            test_info(),
        )?;

        assert_eq!(
            region,
            PixelRegion {
                x: 1,
                y: 1,
                width: 5,
                height: 3,
            }
        );
        Ok(())
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9);
    }
}
