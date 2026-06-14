use super::error::DemError;
use super::{decompress, tile_file};

#[derive(Debug, Clone)]
pub struct DemChunk {
    pub width: u16,
    pub height: u16,
    /// Raw elevation samples in source units (metres). Quantization to the
    /// DEM's storage step happens later, in `compress`.
    pub pixels: Vec<i16>,
}

impl DemChunk {
    /// Decodes a stored `.tengri-dem` tile payload back into a chunk.
    /// Elevations come back quantized to `DEM_QUANTIZATION_METERS`, clamped
    /// to `[0, i16::MAX]`.
    pub fn from_payload(payload: &[u8]) -> Result<Self, DemError> {
        let compressed = tile_file::read_tile(payload)?;
        decompress::decompress_tile(&compressed)
    }

    pub fn from_i16(width: u16, height: u16, pixels: Vec<i16>) -> Self {
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Clamps each sample to `[0, i16::MAX]`. Values above `i16::MAX` saturate;
    /// negatives (bathymetry, nodata sentinels) collapse to `0` — the DEM
    /// pipeline downstream is non-negative.
    pub fn from_i32(width: u16, height: u16, pixels: &[i32]) -> Self {
        let pixels = pixels
            .iter()
            .map(|&value| value.clamp(0, i32::from(i16::MAX)) as i16)
            .collect();
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Rounds each sample, clamps to `[0, i16::MAX]`, and maps non-finite
    /// values to `0`.
    pub fn from_f32(width: u16, height: u16, pixels: &[f32]) -> Self {
        let pixels = pixels
            .iter()
            .map(|&value| {
                if value.is_finite() {
                    value.round().clamp(0.0, f32::from(i16::MAX)) as i16
                } else {
                    0
                }
            })
            .collect();
        Self {
            width,
            height,
            pixels,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressedDemTile {
    pub start: i16,
    pub width: u16,
    pub height: u16,
    pub fixes: Box<[Fix]>,
    pub size_per_delta: u8,
    /// Zstd-compressed packed deltas.
    pub deltas: Box<[u8]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fix {
    pub idx: u16,
    pub elevation: i16,
}
