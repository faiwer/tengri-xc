use std::error::Error;

use super::compress::compress_tile;
use super::constants::DEM_QUANTIZATION_METERS;
use super::decompress::decompress_tile;
use super::tile_file::{read_tile, write_tile};
use super::DemChunk;

#[test]
fn compress_decompress_preserves_quantized_elevations() -> Result<(), Box<dyn Error>> {
    let source = sample_tile();
    let expected = quantized_elevations(&source.pixels);

    let compressed = compress_tile(source)?;
    let decompressed = decompress_tile(&compressed)?;

    assert_eq!(decompressed.width, 256);
    assert_eq!(decompressed.height, 256);
    assert_eq!(decompressed.pixels, expected);

    Ok(())
}

#[test]
fn tile_file_roundtrip_preserves_quantized_elevations() -> Result<(), Box<dyn Error>> {
    let source = sample_tile();
    let expected = quantized_elevations(&source.pixels);

    let compressed = compress_tile(source)?;
    let mut bytes = Vec::new();
    write_tile(&mut bytes, &compressed)?;
    let decoded_tile = read_tile(bytes.as_slice())?;
    let decompressed = decompress_tile(&decoded_tile)?;

    assert_eq!(decompressed.pixels, expected);

    Ok(())
}

#[test]
fn rectangular_compress_decompress_preserves_quantized_elevations() -> Result<(), Box<dyn Error>> {
    let source =
        DemChunk::from_i32(3, 2, &[-5, 0, 7, 12, i32::from(i16::MAX) + 1, 4]);

    let compressed = compress_tile(source)?;
    let decompressed = decompress_tile(&compressed)?;

    assert_eq!(decompressed.width, 3);
    assert_eq!(decompressed.height, 2);
    assert_eq!(decompressed.pixels, vec![0, 0, 8, 16, 32767, 8]);

    Ok(())
}

#[test]
fn rectangular_tile_file_roundtrip_preserves_dimensions() -> Result<(), Box<dyn Error>> {
    let source = DemChunk::from_i16(4, 2, vec![0, 1, 2, 3, 4, 5, 6, 7]);

    let compressed = compress_tile(source)?;
    let mut bytes = Vec::new();
    write_tile(&mut bytes, &compressed)?;
    let decoded_tile = read_tile(bytes.as_slice())?;
    let decompressed = decompress_tile(&decoded_tile)?;

    assert_eq!(decompressed.width, 4);
    assert_eq!(decompressed.height, 2);
    assert_eq!(decompressed.pixels, vec![0, 0, 0, 0, 8, 8, 8, 8]);

    Ok(())
}

#[test]
fn float32_compress_decompress_normalizes_elevations() -> Result<(), Box<dyn Error>> {
    let source =
        DemChunk::from_f32(4, 1, &[f32::NAN, -3.2, 12.6, f32::from(i16::MAX) + 10.0]);

    let compressed = compress_tile(source)?;
    let decompressed = decompress_tile(&compressed)?;

    assert_eq!(decompressed.pixels, vec![0, 0, 16, 32767]);

    Ok(())
}

fn quantized_elevations(pixels: &[i16]) -> Vec<i16> {
    pixels
        .iter()
        .map(|&elevation| quantize(f64::from(elevation.max(0))))
        .collect()
}

fn quantize(elevation: f64) -> i16 {
    let quantization_meters = f64::from(DEM_QUANTIZATION_METERS);
    ((elevation / quantization_meters).round() * quantization_meters)
        .clamp(0.0, f64::from(i16::MAX)) as i16
}

fn sample_tile() -> DemChunk {
    DemChunk::from_i16(
        256,
        256,
        (0..256 * 256).map(|idx| (idx % 1024) as i16 - 20).collect(),
    )
}
