use std::io::{BufReader, Cursor};

use jpeg_decoder::{Decoder as JpegDecoder, PixelFormat};
use png::{ColorType, Decoder as PngDecoder, Transformations};

use crate::matrix::Raster;
use crate::tree::TileTreeError;

/// Decode a PNG byte stream into a 3-channel RGB [`Raster`]. Alpha is stripped
/// (same convention as [`crate::webp::decode::decode_webp_bytes`]). 16-bit depth
/// is downsampled to 8-bit via `STRIP_16`. Greyscale inputs are rejected — they'd
/// triple the archive footprint after WebP re-encode and don't occur in the
/// satellite/orthophoto tile sources this code is built for.
pub(crate) fn decode_png_bytes(bytes: &[u8]) -> Result<Raster, TileTreeError> {
    let mut decoder = PngDecoder::new(Cursor::new(bytes));
    decoder.set_transformations(
        Transformations::EXPAND | Transformations::STRIP_16,
    );
    let mut reader = decoder.read_info()?;
    let info = reader.info().clone();
    let (width, height) = (info.width, info.height);
    if width == 0 || height == 0 || width > u32::from(u16::MAX) || height > u32::from(u16::MAX) {
        return Err(TileTreeError::CorruptFile(
            "PNG dimensions outside u16 range",
        ));
    }

    let output_size = reader
        .output_buffer_size()
        .ok_or(TileTreeError::CorruptFile("PNG output buffer too large"))?;
    let mut buffer = vec![0u8; output_size];
    let frame = reader.next_frame(&mut buffer)?;
    buffer.truncate(frame.buffer_size());

    let pixel_count = (width as usize) * (height as usize);
    let pixels = match frame.color_type {
        ColorType::Rgb => buffer,
        ColorType::Rgba => {
            if buffer.len() != pixel_count * 4 {
                return Err(TileTreeError::CorruptFile(
                    "PNG decode produced unexpected RGBA byte count",
                ));
            }
            let mut rgb = Vec::with_capacity(pixel_count * 3);
            for chunk in buffer.chunks_exact(4) {
                rgb.extend_from_slice(&chunk[..3]);
            }
            rgb
        }
        ColorType::Grayscale | ColorType::GrayscaleAlpha => {
            return Err(TileTreeError::Unsupported(
                "PNG: greyscale tiles rejected (would triple archive size on RGB re-encode)",
            ));
        }
        ColorType::Indexed => {
            return Err(TileTreeError::CorruptFile(
                "PNG palette colour survived EXPAND; expected paletted -> RGB",
            ));
        }
    };

    Ok(Raster {
        width: width as u16,
        height: height as u16,
        channels: 3,
        pixels,
    })
}

/// Decode a JPEG byte stream into a 3-channel RGB [`Raster`]. Only RGB24 inputs
/// are accepted; greyscale (`L8`/`L16`) and `CMYK32` are rejected — greyscale
/// would triple the archive footprint after WebP re-encode, and CMYK would need
/// a colour-management conversion we don't carry. Satellite/orthophoto tiles in
/// the wild are always RGB24.
pub(crate) fn decode_jpeg_bytes(bytes: &[u8]) -> Result<Raster, TileTreeError> {
    let mut decoder = JpegDecoder::new(BufReader::new(Cursor::new(bytes)));
    let pixels = decoder.decode()?;
    let info = decoder.info().ok_or(TileTreeError::CorruptFile(
        "JPEG decode produced no metadata",
    ))?;

    let width = u32::from(info.width);
    let height = u32::from(info.height);
    if width == 0 || height == 0 {
        return Err(TileTreeError::CorruptFile("JPEG has zero dimensions"));
    }
    let pixel_count = (width as usize) * (height as usize);
    match info.pixel_format {
        PixelFormat::RGB24 => {
            if pixels.len() != pixel_count * 3 {
                return Err(TileTreeError::CorruptFile(
                    "JPEG decode produced unexpected RGB byte count",
                ));
            }
        }
        PixelFormat::L8 | PixelFormat::L16 | PixelFormat::CMYK32 => {
            return Err(TileTreeError::Unsupported(
                "JPEG: only RGB24 accepted (greyscale would triple archive size; CMYK needs ICC)",
            ));
        }
    }

    Ok(Raster {
        width: width as u16,
        height: height as u16,
        channels: 3,
        pixels,
    })
}
