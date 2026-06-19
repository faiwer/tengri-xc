use std::io::{BufReader, Cursor};

use image_webp::WebPDecoder;

use crate::matrix::Raster;
use crate::tree::TileTreeError;

/// Decode a standalone WebP byte stream into a 3-channel RGB [`Raster`].
///
/// Alpha is stripped: `tengri-map` ships satellite imagery, where every pixel
/// is opaque and an alpha plane is dead weight (~25 % bytes). The only
/// candidates for real transparency in our pipeline (hillshade, weather,
/// labels) are either out of scope or served from a different path (DEM-derived
/// hillshade), so we drop alpha unconditionally rather than letting RGBA bytes
/// leak into the archive.
pub(crate) fn decode_webp_bytes(bytes: &[u8]) -> Result<Raster, TileTreeError> {
    let mut decoder = WebPDecoder::new(BufReader::new(Cursor::new(bytes)))?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || width > u32::from(u16::MAX) || height > u32::from(u16::MAX) {
        return Err(TileTreeError::CorruptFile(
            "WebP dimensions outside u16 range",
        ));
    }

    let buffer_size = decoder
        .output_buffer_size()
        .ok_or(TileTreeError::CorruptFile("WebP output buffer too large"))?;
    let has_alpha = decoder.has_alpha();
    let mut buffer = vec![0u8; buffer_size];
    decoder.read_image(&mut buffer)?;

    let pixel_count = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().map(|h| w * h))
        .ok_or(TileTreeError::CorruptFile("WebP dimensions overflow usize"))?;
    let pixels = if has_alpha {
        if buffer.len() != pixel_count * 4 {
            return Err(TileTreeError::CorruptFile(
                "WebP decode produced unexpected RGBA byte count",
            ));
        }
        let mut rgb = Vec::with_capacity(pixel_count * 3);
        for chunk in buffer.chunks_exact(4) {
            rgb.extend_from_slice(&chunk[..3]);
        }
        rgb
    } else {
        if buffer.len() != pixel_count * 3 {
            return Err(TileTreeError::CorruptFile(
                "WebP decode produced unexpected RGB byte count",
            ));
        }
        buffer
    };

    Ok(Raster {
        width: width as u16,
        height: height as u16,
        channels: 3,
        pixels,
    })
}
