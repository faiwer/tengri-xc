use crate::tif::{TifPixelMatrix, TiffReadError};

use super::types::{PixelRegion, TiledTifChunk, TiledTifInfo};

pub(super) fn copy_chunk_slice(
    chunk: &TiledTifChunk,
    info: TiledTifInfo,
    region: PixelRegion,
    output: &mut [i16],
) {
    let chunk_x = chunk.tile_x * info.tile_width;
    let chunk_y = chunk.tile_y * info.tile_height;
    let source_x0 = region.x.max(chunk_x);
    let source_y0 = region.y.max(chunk_y);
    let source_x1 = (region.x + region.width).min(chunk_x + chunk.width);
    let source_y1 = (region.y + region.height).min(chunk_y + chunk.height);

    if source_x0 >= source_x1 || source_y0 >= source_y1 {
        return;
    }

    let copy_width = (source_x1 - source_x0) as usize;
    let region_width = region.width as usize;
    let chunk_width = chunk.width as usize;

    for source_y in source_y0..source_y1 {
        let chunk_row = (source_y - chunk_y) as usize;
        let chunk_col = (source_x0 - chunk_x) as usize;
        let output_row = (source_y - region.y) as usize;
        let output_col = (source_x0 - region.x) as usize;
        let source_idx = chunk_row * chunk_width + chunk_col;
        let output_idx = output_row * region_width + output_col;

        match &chunk.pixels {
            TifPixelMatrix::I16(pixels) => {
                output[output_idx..output_idx + copy_width]
                    .copy_from_slice(&pixels[source_idx..source_idx + copy_width]);
            }
            TifPixelMatrix::I32(pixels) => {
                for offset in 0..copy_width {
                    output[output_idx + offset] =
                        normalize_elevation(i64::from(pixels[source_idx + offset]));
                }
            }
            TifPixelMatrix::F32(pixels) => {
                for offset in 0..copy_width {
                    output[output_idx + offset] =
                        normalize_float_elevation(pixels[source_idx + offset]);
                }
            }
        }
    }
}

pub(super) fn pixel_count(width: u32, height: u32) -> Result<usize, TiffReadError> {
    let width = usize::try_from(width).map_err(|_| TiffReadError::ImageTooLarge)?;
    let height = usize::try_from(height).map_err(|_| TiffReadError::ImageTooLarge)?;
    width
        .checked_mul(height)
        .ok_or(TiffReadError::ImageTooLarge)
}

fn normalize_elevation(elevation: i64) -> i16 {
    i16::try_from(elevation).unwrap_or(i16::MAX).max(0)
}

fn normalize_float_elevation(elevation: f32) -> i16 {
    if !elevation.is_finite() {
        return 0;
    }

    elevation.round().clamp(0.0, f32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use crate::geo::Bounds;

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
    fn copy_adjacent_tile_slices_into_output() {
        let info = test_info();
        let region = PixelRegion {
            x: 2,
            y: 1,
            width: 4,
            height: 2,
        };
        let left = TiledTifChunk {
            tile_x: 0,
            tile_y: 0,
            width: 4,
            height: 4,
            pixels: TifPixelMatrix::I16((0..16).collect()),
        };
        let right = TiledTifChunk {
            tile_x: 1,
            tile_y: 0,
            width: 4,
            height: 4,
            pixels: TifPixelMatrix::I16((100..116).collect()),
        };
        let mut output = vec![-1; 8];

        copy_chunk_slice(&left, info, region, &mut output);
        copy_chunk_slice(&right, info, region, &mut output);

        assert_eq!(output, vec![6, 7, 104, 105, 10, 11, 108, 109]);
    }

    #[test]
    fn i32_source_values_are_normalized_to_i16() {
        let info = test_info();
        let region = PixelRegion {
            x: 0,
            y: 0,
            width: 3,
            height: 1,
        };
        let chunk = TiledTifChunk {
            tile_x: 0,
            tile_y: 0,
            width: 4,
            height: 1,
            pixels: TifPixelMatrix::I32(vec![-5, 7, i32::MAX, 9]),
        };
        let mut output = vec![-1; 3];

        copy_chunk_slice(&chunk, info, region, &mut output);

        assert_eq!(output, vec![0, 7, i16::MAX]);
    }

    #[test]
    fn f32_source_values_are_normalized_to_i16() {
        let info = test_info();
        let region = PixelRegion {
            x: 0,
            y: 0,
            width: 4,
            height: 1,
        };
        let chunk = TiledTifChunk {
            tile_x: 0,
            tile_y: 0,
            width: 4,
            height: 1,
            pixels: TifPixelMatrix::F32(vec![f32::NAN, -2.0, 12.6, f32::from(i16::MAX) + 10.0]),
        };
        let mut output = vec![-1; 4];

        copy_chunk_slice(&chunk, info, region, &mut output);

        assert_eq!(output, vec![0, 0, 13, i16::MAX]);
    }
}
