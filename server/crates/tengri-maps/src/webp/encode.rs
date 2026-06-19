use ::webp::Encoder;

use crate::matrix::Raster;
use crate::tree::TileTreeError;

/// Lossy WebP encode of a 3-channel RGB raster.
///
/// `quality` is the libwebp 0..=100 lossy knob (75 is libwebp's default). Other
/// encoder parameters (method, pass, sns_strength, …) stay at libwebp's
/// defaults — promote them to flags only when there's a real tuning need.
///
/// Alpha is not supported: the matrix path strips it at decode (see
/// [`crate::webp::decode::decode_webp_bytes`]) and the passthrough path rejects
/// RGBA-shipping sources, so the archive is uniformly RGB. A 4-channel input
/// here is a contract bug.
pub(super) fn encode_lossy(raster: &Raster, quality: u8) -> Result<Vec<u8>, TileTreeError> {
    if raster.channels != 3 {
        return Err(TileTreeError::Unsupported(
            "WebP encode requires 3 (RGB) channels; alpha is dropped at decode",
        ));
    }
    let q = f32::from(quality.min(100));
    let width = u32::from(raster.width);
    let height = u32::from(raster.height);
    let memory = Encoder::from_rgb(&raster.pixels, width, height).encode(q);
    Ok(memory.to_vec())
}
