use std::mem;

use super::bitpack::BitWriter;
use super::error::DemError;
use super::types::{CompressedDemTile, Fix};
use crate::constants::{
    DEM_QUANTIZATION_METERS, MAX_DELTA_BITS, MAX_DEM_TILE_SIDE, MIN_DELTA_BITS,
};
use crate::tif::{TifPixelMatrix, TifChunk};

const DELTA_WIDTH_BUCKETS: usize = (MAX_DELTA_BITS - MIN_DELTA_BITS + 1) as usize;
const OUTLIER_BUCKET: usize = DELTA_WIDTH_BUCKETS;
const DELTA_BUCKETS: usize = DELTA_WIDTH_BUCKETS + 1;
const ZSTD_LEVEL: i32 = 3;

pub fn compress_tile(source: TifChunk) -> Result<CompressedDemTile, DemError> {
    let (width, height) = tile_dimensions(source.width, source.height)?;
    let elevations = elevations_from_pixels(source.pixels)?;
    let expected = pixel_count(source.width, source.height)?;
    if elevations.len() != expected {
        return Err(DemError::UnexpectedPixelCount {
            expected,
            actual: elevations.len(),
        });
    }

    let start = elevations[0];
    let source_deltas = source_deltas(&elevations, source.width as usize);
    let size_per_delta = choose_size_per_delta(&source_deltas);
    let encoded = encode_with_delta_size(&elevations, &source_deltas, size_per_delta);
    let deltas = zstd(&encoded.deltas)?;

    Ok(CompressedDemTile {
        start,
        width,
        height,
        fixes: encoded.fixes.into_boxed_slice(),
        size_per_delta,
        deltas: deltas.into_boxed_slice(),
    })
}

fn tile_dimensions(width: u16, height: u16) -> Result<(u16, u16), DemError> {
    if width == 0 || height == 0 || width > MAX_DEM_TILE_SIDE || height > MAX_DEM_TILE_SIDE {
        return Err(DemError::UnsupportedDimensions {
            width: u32::from(width),
            height: u32::from(height),
        });
    }

    Ok((width, height))
}

fn pixel_count(width: u16, height: u16) -> Result<usize, DemError> {
    usize::from(width)
        .checked_mul(usize::from(height))
        .ok_or(DemError::UnsupportedDimensions {
            width: u32::from(width),
            height: u32::from(height),
        })
}

fn elevations_from_pixels(pixels: TifPixelMatrix) -> Result<Vec<i16>, DemError> {
    match pixels {
        TifPixelMatrix::I16(pixels) => Ok(pixels
            .into_iter()
            .map(|elevation| normalize_elevation(i64::from(elevation)))
            .collect()),
        TifPixelMatrix::I32(pixels) => Ok(pixels
            .into_iter()
            .map(|elevation| normalize_elevation(i64::from(elevation)))
            .collect()),
        TifPixelMatrix::F32(pixels) => Ok(pixels.into_iter().map(normalize_float_elevation).collect()),
    }
}

fn normalize_elevation(elevation: i64) -> i16 {
    quantize_elevation(elevation.clamp(0, i64::from(i16::MAX)) as f64)
}

fn normalize_float_elevation(elevation: f32) -> i16 {
    if !elevation.is_finite() {
        return 0;
    }

    quantize_elevation(f64::from(elevation).clamp(0.0, f64::from(i16::MAX)))
}

fn quantize_elevation(elevation: f64) -> i16 {
    (elevation / f64::from(DEM_QUANTIZATION_METERS)).round() as i16
}

fn source_deltas(elevations: &[i16], dimension: usize) -> Vec<i16> {
    (1..elevations.len())
        .map(|idx| {
            let reference_idx = if idx % dimension == 0 {
                idx - dimension
            } else {
                idx - 1
            };
            elevations[idx] - elevations[reference_idx]
        })
        .collect()
}

fn choose_size_per_delta(deltas: &[i16]) -> u8 {
    let mut minimum_width_counts = [0usize; DELTA_BUCKETS];
    for &delta in deltas {
        let bucket = minimum_signed_width(delta)
            .map(delta_width_bucket)
            .unwrap_or(OUTLIER_BUCKET);
        minimum_width_counts[bucket] += 1;
    }

    let mut best_width = MIN_DELTA_BITS;
    let mut best_size = usize::MAX;
    let fix_size = mem::size_of::<Fix>();
    let mut inliers = 0;

    for width in MIN_DELTA_BITS..=MAX_DELTA_BITS {
        inliers += minimum_width_counts[delta_width_bucket(width)];
        let outliers = deltas.len() - inliers;
        let estimated_size = (inliers * usize::from(width)).div_ceil(8) + outliers * fix_size;

        if estimated_size < best_size {
            best_width = width;
            best_size = estimated_size;
        }
    }

    best_width
}

fn minimum_signed_width(value: i16) -> Option<u8> {
    (MIN_DELTA_BITS..=MAX_DELTA_BITS).find(|&width| fits_signed_width(value, width))
}

fn delta_width_bucket(width: u8) -> usize {
    usize::from(width - MIN_DELTA_BITS)
}

struct EncodedCandidate {
    fixes: Vec<Fix>,
    deltas: Vec<u8>,
}

fn encode_with_delta_size(
    elevations: &[i16],
    source_deltas: &[i16],
    size_per_delta: u8,
) -> EncodedCandidate {
    let mut bit_writer = BitWriter::new();
    let mut fixes = Vec::new();

    for (delta_idx, &delta) in source_deltas.iter().enumerate() {
        let idx = delta_idx + 1;
        if fits_signed_width(delta, size_per_delta) {
            bit_writer.push_signed(delta, size_per_delta);
        } else {
            fixes.push(Fix {
                idx: idx as u16,
                elevation: elevations[idx],
            });
        }
    }

    EncodedCandidate {
        fixes,
        deltas: bit_writer.finish(),
    }
}

fn fits_signed_width(value: i16, width: u8) -> bool {
    let min = -(1i16 << (width - 1));
    let max = (1i16 << (width - 1)) - 1;
    (min..=max).contains(&value)
}

fn zstd(bytes: &[u8]) -> Result<Vec<u8>, DemError> {
    Ok(zstd::encode_all(bytes, ZSTD_LEVEL)?)
}
