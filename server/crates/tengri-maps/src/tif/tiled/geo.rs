use crate::geo::{
    Bounds, WEB_MERCATOR_HALF_EQUATOR_M, WEB_MERCATOR_MAX_LAT, lat_to_mercator_y_m,
    lon_to_mercator_x_m, mercator_x_m_to_lon, mercator_y_m_to_lat,
};

use crate::tif::error::TiffReadError;

use super::projection::TifProjection;
use super::types::{PixelRegion, TiledTifInfo};

/// Round-to-nearest tolerance, in fractional pixels, for snapping floor/ceil
/// inputs to integer values. Mercator's `asinh(tan(lat))` path introduces
/// ULP-level error that would otherwise turn a pixel-aligned XYZ tile edge into
/// a 1-pixel overshoot.
const PIXEL_SNAP_TOLERANCE: f64 = 1e-6;

fn outward_floor(value: f64) -> f64 {
    let nearest = value.round();
    if (value - nearest).abs() < PIXEL_SNAP_TOLERANCE {
        nearest
    } else {
        value.floor()
    }
}

fn outward_ceil(value: f64) -> f64 {
    let nearest = value.round();
    if (value - nearest).abs() < PIXEL_SNAP_TOLERANCE {
        nearest
    } else {
        value.ceil()
    }
}

/// Convert a geographic [`Bounds`] request into the source pixel rectangle that
/// covers it. For [`TifProjection::Wgs84`] this is direct linear pixel math in
/// degrees; for [`TifProjection::WebMercator`] it forward-projects the four
/// corners through the Mercator transform first and then does the same linear
/// math in projected metres.
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

    let (left, right, top, bottom) = match info.projection {
        TifProjection::Wgs84 => (
            outward_floor((bounds.min_lon - info.origin_x) / info.pixel_width),
            outward_ceil((bounds.max_lon - info.origin_x) / info.pixel_width),
            outward_floor((info.origin_y - bounds.max_lat) / info.pixel_height),
            outward_ceil((info.origin_y - bounds.min_lat) / info.pixel_height),
        ),
        TifProjection::WebMercator => {
            let x_min = lon_to_mercator_x_m(bounds.min_lon);
            let x_max = lon_to_mercator_x_m(bounds.max_lon);
            // `lat_to_mercator_y_m` saturates near the poles, so an XYZ
            // tile reaching above ±85.05° is silently clamped to the
            // world's metric edge — same behaviour as GDAL.
            let y_min = lat_to_mercator_y_m(bounds.min_lat);
            let y_max = lat_to_mercator_y_m(bounds.max_lat);
            (
                outward_floor((x_min - info.origin_x) / info.pixel_width),
                outward_ceil((x_max - info.origin_x) / info.pixel_width),
                outward_floor((info.origin_y - y_max) / info.pixel_height),
                outward_ceil((info.origin_y - y_min) / info.pixel_height),
            )
        }
    };

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

/// Geographic extent of the source raster in lat/lon degrees.
///
/// For [`TifProjection::Wgs84`] this is a straightforward "origin minus N
/// pixels". For [`TifProjection::WebMercator`] the metric corners are
/// inverse-projected through the Mercator transform, then clamped at
/// `±WEB_MERCATOR_MAX_LAT` because the projection diverges at the poles and a 1
/// px overshoot in metres can map to nonsense lat values.
pub(super) fn source_bounds(
    width: u32,
    height: u32,
    projection: TifProjection,
    origin_x: f64,
    origin_y: f64,
    pixel_width: f64,
    pixel_height: f64,
) -> Bounds {
    match projection {
        TifProjection::Wgs84 => Bounds {
            min_lat: origin_y - f64::from(height) * pixel_height,
            min_lon: origin_x,
            max_lat: origin_y,
            max_lon: origin_x + f64::from(width) * pixel_width,
        },
        TifProjection::WebMercator => {
            let max_y_m = origin_y;
            let min_y_m = origin_y - f64::from(height) * pixel_height;
            let min_x_m = origin_x;
            let max_x_m = origin_x + f64::from(width) * pixel_width;

            let max_lat = mercator_y_m_to_lat(max_y_m.clamp(
                -WEB_MERCATOR_HALF_EQUATOR_M,
                WEB_MERCATOR_HALF_EQUATOR_M,
            ));
            let min_lat = mercator_y_m_to_lat(min_y_m.clamp(
                -WEB_MERCATOR_HALF_EQUATOR_M,
                WEB_MERCATOR_HALF_EQUATOR_M,
            ));
            Bounds {
                min_lat: min_lat.max(-WEB_MERCATOR_MAX_LAT),
                min_lon: mercator_x_m_to_lon(min_x_m),
                max_lat: max_lat.min(WEB_MERCATOR_MAX_LAT),
                max_lon: mercator_x_m_to_lon(max_x_m),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wgs84_test_info() -> TiledTifInfo {
        TiledTifInfo {
            width: 8,
            height: 4,
            tile_width: 4,
            tile_height: 4,
            tiles_across: 2,
            tiles_down: 1,
            projection: TifProjection::Wgs84,
            origin_x: 0.0,
            origin_y: 4.0,
            pixel_width: 1.0,
            pixel_height: 1.0,
            bounds: Bounds {
                min_lat: 0.0,
                min_lon: 0.0,
                max_lat: 4.0,
                max_lon: 8.0,
            },
        }
    }

    /// 32768 px square, full Web-Mercator world, ~1.22 km/px — the layout the
    /// orchestrator's `tests/build_z6_tif.py` Mercator path produces.
    fn full_world_mercator_info() -> TiledTifInfo {
        let side = 32768u32;
        let pixel = (2.0 * WEB_MERCATOR_HALF_EQUATOR_M) / f64::from(side);
        TiledTifInfo {
            width: side,
            height: side,
            tile_width: 256,
            tile_height: 256,
            tiles_across: side / 256,
            tiles_down: side / 256,
            projection: TifProjection::WebMercator,
            origin_x: -WEB_MERCATOR_HALF_EQUATOR_M,
            origin_y: WEB_MERCATOR_HALF_EQUATOR_M,
            pixel_width: pixel,
            pixel_height: pixel,
            bounds: Bounds {
                min_lat: -WEB_MERCATOR_MAX_LAT,
                min_lon: -180.0,
                max_lat: WEB_MERCATOR_MAX_LAT,
                max_lon: 180.0,
            },
        }
    }

    #[test]
    fn source_bounds_come_from_origin_scale_and_dimensions() {
        let bounds = source_bounds(
            3600,
            3600,
            TifProjection::Wgs84,
            10.0,
            48.0,
            1.0 / 3600.0,
            1.0 / 3600.0,
        );

        assert_close(bounds.min_lat, 47.0);
        assert_close(bounds.min_lon, 10.0);
        assert_close(bounds.max_lat, 48.0);
        assert_close(bounds.max_lon, 11.0);
    }

    #[test]
    fn web_mercator_full_world_bounds_clamp_at_pole() {
        let info = full_world_mercator_info();
        let bounds = source_bounds(
            info.width,
            info.height,
            info.projection,
            info.origin_x,
            info.origin_y,
            info.pixel_width,
            info.pixel_height,
        );

        assert_close(bounds.min_lon, -180.0);
        assert_close(bounds.max_lon, 180.0);
        assert_close(bounds.min_lat, -WEB_MERCATOR_MAX_LAT);
        assert_close(bounds.max_lat, WEB_MERCATOR_MAX_LAT);
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
            wgs84_test_info(),
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

    /// At z=6, an XYZ tile is exactly one 64th of the world per side. With a
    /// 32768 px Mercator source that's 512 px per side regardless of where
    /// the tile sits — the projection grids are aligned by construction.
    #[test]
    fn web_mercator_z6_tile_bounds_resolve_to_a_512_px_square() -> Result<(), TiffReadError> {
        let info = full_world_mercator_info();
        let z6_tile_bounds = crate::geo::xyz_tile_bounds(6, 32, 32).unwrap();

        let region = pixel_region_for_bounds(z6_tile_bounds, info)?;

        assert_eq!(region.width, 512);
        assert_eq!(region.height, 512);
        assert_eq!(region.x, 16384); // tile (32, 32) starts at the world's centre x
        assert_eq!(region.y, 16384); //                              and centre y
        Ok(())
    }

    /// At lat ≈ 60° N the same z=6 tile *width* is still exactly 512 px
    /// (longitudinal slice is uniform in metres), but its *height* in
    /// projected metres is bigger than at the equator because Mercator
    /// stretches with latitude. The pixel-region math stays linear and
    /// round-trips outward-rounded.
    #[test]
    fn web_mercator_polar_tile_picks_a_metric_strip() -> Result<(), TiffReadError> {
        let info = full_world_mercator_info();
        // Web-Mercator y for a tile at z=6, y=20 sits around lat 60° N.
        let bounds = crate::geo::xyz_tile_bounds(6, 32, 20).unwrap();

        let region = pixel_region_for_bounds(bounds, info)?;

        // x is exact: tile 32 of 64 starts at world centre, so x=16384.
        assert_eq!(region.width, 512);
        assert_eq!(region.x, 16384);
        // y projects through the log/tan, but a z=6 tile is still exactly
        // 1/64 of the metric world in y — so it lands on 512 source rows.
        assert_eq!(region.height, 512);
        Ok(())
    }

    /// A Mercator request whose bounds expand past the world's metric edge
    /// (an XYZ tile reaching above ±85.05°) gets clamped at the pole and
    /// stays inside the raster instead of failing with `RegionOutOfBounds`.
    #[test]
    fn web_mercator_request_at_extreme_pole_is_clamped() -> Result<(), TiffReadError> {
        let info = full_world_mercator_info();
        let z6_tile_y0 = crate::geo::xyz_tile_bounds(6, 0, 0).unwrap();

        let region = pixel_region_for_bounds(z6_tile_y0, info)?;
        assert_eq!(region.width, 512);
        assert_eq!(region.height, 512);
        assert_eq!(region.x, 0);
        assert_eq!(region.y, 0);
        Ok(())
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "{actual} != {expected}",
        );
    }
}
