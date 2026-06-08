use std::error::Error;

use tengri_maps::dem::{compress_tile, decompress_tile};
use tengri_maps::tiff::{PixelMatrix, read_single_channel_tiff};


#[test]
fn compress_decompress_preserves_elevations() -> Result<(), Box<dyn Error>> {
    let source = read_single_channel_tiff(SAMPLE_TILE)?;
    let expected = normalized_elevations(&source.pixels);

    let compressed = compress_tile(source)?;
    let decompressed = decompress_tile(&compressed)?;

    assert_eq!(decompressed.dimension, 256);
    assert_eq!(decompressed.elevations.as_ref(), expected.as_slice());

    Ok(())
}

fn normalized_elevations(pixels: &PixelMatrix) -> Vec<u16> {
    match pixels {
        PixelMatrix::I16(pixels) => pixels
            .iter()
            .map(|&elevation| elevation.max(0) as u16)
            .collect(),
        PixelMatrix::I32(pixels) => pixels
            .iter()
            .map(|&elevation| elevation.clamp(0, i32::from(i16::MAX)) as u16)
            .collect(),
    }
}
