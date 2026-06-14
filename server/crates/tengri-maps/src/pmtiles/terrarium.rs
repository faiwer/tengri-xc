use std::io::{BufReader, Cursor};

use image_webp::WebPDecoder;

use crate::{
    dem::{DemChunk, DemPixelMatrix},
    tree::TileTreeError,
};
use super::constants::MAX_SOURCE_TILE_SIDE;

pub fn decode_terrarium_webp(bytes: &[u8]) -> Result<DemChunk, TileTreeError> {
    let mut decoder = WebPDecoder::new(BufReader::new(Cursor::new(bytes)))?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || width > MAX_SOURCE_TILE_SIDE || height > MAX_SOURCE_TILE_SIDE {
        return Err(TileTreeError::CorruptFile(
            "PMTiles DEM tile has unsupported dimensions",
        ));
    }

    let buffer_size = decoder
        .output_buffer_size()
        .ok_or(TileTreeError::CorruptFile(
            "PMTiles DEM tile output buffer is too large",
        ))?;
    let mut rgba_or_rgb = vec![0; buffer_size];
    let stride = if decoder.has_alpha() { 4 } else { 3 };
    decoder.read_image(&mut rgba_or_rgb)?;

    let pixel_count = usize::try_from(width)
        .ok()
        .and_then(|width| usize::try_from(height).ok().map(|height| width * height))
        .ok_or(TileTreeError::CorruptFile("PMTiles DEM tile is too large"))?;
    if rgba_or_rgb.len() != pixel_count * stride {
        return Err(TileTreeError::CorruptFile(
            "PMTiles DEM tile decoded to an unexpected byte count",
        ));
    }

    let mut pixels = Vec::with_capacity(pixel_count);
    for pixel in rgba_or_rgb.chunks_exact(stride) {
        if stride == 4 && pixel[3] == 0 {
            pixels.push(i16::MIN);
            continue;
        }

        pixels.push(terrarium_elevation(pixel[0], pixel[1], pixel[2]));
    }

    Ok(DemChunk {
        width: width as u16,
        height: height as u16,
        pixels: DemPixelMatrix::I16(pixels),
    })
}

fn terrarium_elevation(red: u8, green: u8, blue: u8) -> i16 {
    let elevation = f64::from(red) * 256.0 + f64::from(green) + f64::from(blue) / 256.0 - 32768.0;
    elevation
        .round()
        .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrarium_elevation_decodes_zero_meters() {
        assert_eq!(terrarium_elevation(128, 0, 0), 0);
    }

    #[test]
    fn terrarium_elevation_rounds_fractional_meters() {
        assert_eq!(terrarium_elevation(128, 1, 128), 2);
    }
}