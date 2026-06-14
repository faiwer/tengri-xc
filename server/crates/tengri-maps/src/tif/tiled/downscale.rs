use crate::dem::constants::MAX_DEM_TILE_SIDE;
use crate::matrix::area_resample;
use crate::tif::error::TiffReadError;

use super::types::PixelRegion;

const MAX_EXACT_REGION_SIDE: u32 = MAX_DEM_TILE_SIDE as u32 * 2;

pub(super) fn downscale_to_dem_tile(region: PixelRegion, pixels: Vec<i16>) -> (u16, u16, Vec<i16>) {
    let width = region.width.min(u32::from(MAX_DEM_TILE_SIDE)) as u16;
    let height = region.height.min(u32::from(MAX_DEM_TILE_SIDE)) as u16;
    if region.width == u32::from(width) && region.height == u32::from(height) {
        return (width, height, pixels);
    }

    (
        width,
        height,
        downscale_pixels(
            &pixels,
            region.width as usize,
            region.height as usize,
            width,
            height,
        ),
    )
}

pub(super) fn validate_exact_region_dimensions(region: PixelRegion) -> Result<(), TiffReadError> {
    if region.width == 0
        || region.height == 0
        || region.width > MAX_EXACT_REGION_SIDE
        || region.height > MAX_EXACT_REGION_SIDE
    {
        return Err(TiffReadError::ImageTooLarge);
    }

    Ok(())
}

fn downscale_pixels(
    source: &[i16],
    source_width: usize,
    source_height: usize,
    width: u16,
    height: u16,
) -> Vec<i16> {
    area_resample(
        source,
        source_width,
        source_height,
        usize::from(width),
        usize::from(height),
        |value| f64::from(value.max(0)),
    )
    .into_iter()
    .map(|value| value.round() as i16)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_region_dimensions_are_capped_before_downscale() {
        let result = validate_exact_region_dimensions(PixelRegion {
            x: 0,
            y: 0,
            width: 513,
            height: 1,
        });

        assert!(matches!(result, Err(TiffReadError::ImageTooLarge)));
    }

    #[test]
    fn dem_downscale_is_noop_when_region_already_fits() {
        let pixels = vec![1, 2, 3, 4, 5, 6];
        let (width, height, output) = downscale_to_dem_tile(
            PixelRegion {
                x: 0,
                y: 0,
                width: 3,
                height: 2,
            },
            pixels.clone(),
        );

        assert_eq!(width, 3);
        assert_eq!(height, 2);
        assert_eq!(output, pixels);
    }

    #[test]
    fn dem_downscale_caps_only_oversized_dimension_with_area_average() {
        let source_width = 512usize;
        let source_height = 1usize;
        let pixels: Vec<i16> = (0..source_width * source_height)
            .map(|idx| idx as i16)
            .collect();

        let (width, height, output) = downscale_to_dem_tile(
            PixelRegion {
                x: 0,
                y: 0,
                width: source_width as u32,
                height: source_height as u32,
            },
            pixels.clone(),
        );

        assert_eq!(width, 256);
        assert_eq!(height, 1);
        assert_eq!(output[0], 1);
        assert_eq!(output[1], 3);
        assert_eq!(output[255], 511);
    }
}
