use crate::dem::constants::MAX_DEM_TILE_SIDE;
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
    let width = usize::from(width);
    let height = usize::from(height);
    let mut output = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            output.push(area_average(
                source,
                source_width,
                source_height,
                x,
                y,
                width,
                height,
            ));
        }
    }
    output
}

fn area_average(
    source: &[i16],
    source_width: usize,
    source_height: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> i16 {
    let x_range = source_range(x, width, source_width);
    let y_range = source_range(y, height, source_height);
    let x_start = x_range.0.floor() as usize;
    let x_end = x_range.1.ceil() as usize;
    let y_start = y_range.0.floor() as usize;
    let y_end = y_range.1.ceil() as usize;
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;

    for source_y in y_start..y_end.min(source_height) {
        let y_weight = overlap(y_range, source_y);
        for source_x in x_start..x_end.min(source_width) {
            let weight = overlap(x_range, source_x) * y_weight;
            weighted_sum += f64::from(source[source_y * source_width + source_x].max(0)) * weight;
            weight_sum += weight;
        }
    }

    (weighted_sum / weight_sum).round() as i16
}

fn source_range(output_idx: usize, output_len: usize, source_len: usize) -> (f64, f64) {
    let scale = source_len as f64 / output_len as f64;
    let start = output_idx as f64 * scale;
    (start, start + scale)
}

fn overlap(range: (f64, f64), idx: usize) -> f64 {
    let start = range.0.max(idx as f64);
    let end = range.1.min(idx as f64 + 1.0);
    (end - start).max(0.0)
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
