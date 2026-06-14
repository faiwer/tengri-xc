use super::constants::MAX_DEM_TILE_SIDE;
use super::error::DemError;
use super::DemChunk;
use crate::geo::Bounds;
use crate::matrix::area_resample;

pub const MIN_DEM_TILE_SIDE: u16 = 16;

/// Pixel dimensions for a stored DEM tile, after latitude-aware downscale.
///
/// The source DEM is geographic (uniform pixels in degrees), but the tile is
/// served on a Web Mercator grid where one degree of longitude shrinks by
/// `cos(lat)` toward the poles. Storing the source resolution unchanged would
/// waste bytes on horizontal samples no renderer can resolve, so the width is
/// scaled down by `cos(lat)`; the height is left at the raw count. Both sides
/// are clamped to `[MIN_DEM_TILE_SIDE, MAX_DEM_TILE_SIDE]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemTileResolution {
    pub width: u16,
    pub height: u16,
}

pub fn target_dem_resolution(bounds: Bounds, raw_width: u16, raw_height: u16) -> DemTileResolution {
    let center_lat = (bounds.min_lat + bounds.max_lat) / 2.0;
    let lng_scale = center_lat.to_radians().cos().abs();

    DemTileResolution {
        width: target_side(raw_width, lng_scale),
        height: target_side(raw_height, 1.0),
    }
}

pub fn resize_dem_matrix(
    source: DemChunk,
    target: DemTileResolution,
) -> Result<DemChunk, DemError> {
    validate_matrix_len(&source)?;
    if source.width == target.width && source.height == target.height {
        return Ok(source);
    }

    let pixels = area_resample(
        &source.pixels,
        usize::from(source.width),
        usize::from(source.height),
        usize::from(target.width),
        usize::from(target.height),
        |value| f64::from(value.max(0)),
    )
    .into_iter()
    .map(|value| value.round().clamp(0.0, f64::from(i16::MAX)) as i16)
    .collect();
    Ok(DemChunk {
        width: target.width,
        height: target.height,
        pixels,
    })
}

fn target_side(raw: u16, scale: f64) -> u16 {
    ((f64::from(raw) * scale).round() as u16).clamp(MIN_DEM_TILE_SIDE, MAX_DEM_TILE_SIDE)
}

fn validate_matrix_len(source: &DemChunk) -> Result<(), DemError> {
    let expected = usize::from(source.width) * usize::from(source.height);
    let actual = source.pixels.len();
    if expected != actual {
        return Err(DemError::UnexpectedPixelCount { expected, actual });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equator_keeps_width_near_source_width() {
        let target = target_dem_resolution(bounds_around(0.0), 256, 128);

        assert_eq!(
            target,
            DemTileResolution {
                width: 256,
                height: 128,
            }
        );
    }

    #[test]
    fn sixty_degrees_roughly_halves_width() {
        let target = target_dem_resolution(bounds_around(60.0), 256, 128);

        assert_eq!(target.width, 128);
        assert_eq!(target.height, 128);
    }

    #[test]
    fn web_mercator_limit_clamps_width_to_minimum() {
        let target = target_dem_resolution(bounds_around(85.0), 128, 128);

        assert_eq!(target.width, 16);
        assert_eq!(target.height, 128);
    }

    #[test]
    fn resize_supports_non_square_target_dimensions() {
        let source = DemChunk::from_i16(4, 4, (0..16).collect());

        let resized = resize_dem_matrix(
            source,
            DemTileResolution {
                width: 2,
                height: 3,
            },
        )
        .unwrap();

        assert_eq!(resized.width, 2);
        assert_eq!(resized.height, 3);
        assert_eq!(resized.pixels.len(), 6);
    }

    #[test]
    fn resize_can_expand_to_minimum_dimension() {
        let source = DemChunk::from_i16(1, 1, vec![42]);

        let resized = resize_dem_matrix(
            source,
            DemTileResolution {
                width: 16,
                height: 16,
            },
        )
        .unwrap();

        assert_eq!(resized.width, 16);
        assert_eq!(resized.height, 16);
        assert_eq!(resized.pixels.len(), 256);
    }

    fn bounds_around(lat: f64) -> Bounds {
        Bounds {
            min_lat: lat - 0.5,
            min_lon: 0.0,
            max_lat: lat + 0.5,
            max_lon: 1.0,
        }
    }
}
