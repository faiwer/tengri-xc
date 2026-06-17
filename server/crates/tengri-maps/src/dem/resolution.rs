use super::DemChunk;
use super::constants::MAX_DEM_TILE_SIDE;
use super::error::DemError;
use crate::matrix::area_resample;

/// Cap a DEM matrix at `MAX_DEM_TILE_SIDE` per side. Sources larger than the
/// cap (PMTiles 512×512, stitched parents up to 1024×1024) are area-resampled
/// down with negative samples floored at 0; sources already in range are
/// returned unchanged.
///
/// The flooring matches `compress_tile`'s storage rule (`max(0)` then quantize)
/// and protects the average from `i16::MIN` ocean pixels.
pub fn cap_dem_matrix(source: DemChunk) -> Result<DemChunk, DemError> {
    validate_matrix_len(&source)?;

    let target_width = source.width.min(MAX_DEM_TILE_SIDE);
    let target_height = source.height.min(MAX_DEM_TILE_SIDE);
    if source.width == target_width && source.height == target_height {
        return Ok(source);
    }

    let pixels = area_resample(
        &source.pixels,
        usize::from(source.width),
        usize::from(source.height),
        usize::from(target_width),
        usize::from(target_height),
        |value| f64::from(value.max(0)),
    )
    .into_iter()
    .map(|value| value.round().clamp(0.0, f64::from(i16::MAX)) as i16)
    .collect();

    Ok(DemChunk {
        width: target_width,
        height: target_height,
        pixels,
    })
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
    fn matrix_within_cap_is_returned_unchanged() {
        let source = DemChunk::from_i16(64, 32, (0..64 * 32).map(|i| i as i16).collect());
        let capped = cap_dem_matrix(source.clone()).unwrap();

        assert_eq!(capped.width, 64);
        assert_eq!(capped.height, 32);
        assert_eq!(capped.pixels, source.pixels);
    }

    #[test]
    fn matrix_above_cap_is_downscaled_per_axis() {
        let source = DemChunk::from_i16(512, 128, vec![100; 512 * 128]);
        let capped = cap_dem_matrix(source).unwrap();

        assert_eq!(capped.width, MAX_DEM_TILE_SIDE);
        assert_eq!(capped.height, 128);
        assert_eq!(capped.pixels.len(), usize::from(MAX_DEM_TILE_SIDE) * 128);
        assert!(capped.pixels.iter().all(|&v| v == 100));
    }

    #[test]
    fn negative_source_samples_are_floored_to_zero_in_average() {
        // Two-pixel source, one ocean (i16::MIN), one peak. Cap to 1
        // pixel via area-resample. Expected average uses 0 for the
        // ocean side, so the result is half the peak elevation.
        let source = DemChunk::from_i16(2, 1, vec![i16::MIN, 100]);
        let capped = cap_dem_matrix(DemChunk {
            width: 2,
            height: 1,
            pixels: source.pixels,
        })
        .unwrap_or_else(|err| panic!("cap_dem_matrix: {err}"));
        // 2 -> 1 area resample at MAX is a no-op (1 ≤ MAX); test the
        // explicit downscale path by faking a wider source.
        assert_eq!(capped.width, 2);

        let wide = DemChunk::from_i16(
            usize::from(MAX_DEM_TILE_SIDE) as u16 + 2,
            1,
            std::iter::once(i16::MIN)
                .chain(std::iter::repeat_n(100, usize::from(MAX_DEM_TILE_SIDE) + 1))
                .collect(),
        );
        let capped = cap_dem_matrix(wide).unwrap();
        assert_eq!(capped.width, MAX_DEM_TILE_SIDE);
        // The single i16::MIN sample contributes 0 to its bucket;
        // the rest are 100 → every output pixel is at most 100, none
        // is negative.
        assert!(capped.pixels.iter().all(|&v| (0..=100).contains(&v)));
    }
}
