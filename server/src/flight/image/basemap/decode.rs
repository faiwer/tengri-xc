//! Tile JPEG bytes to a tiny-skia pixmap.

use jpeg_decoder::{Decoder, PixelFormat};
use tiny_skia::{Pixmap, PremultipliedColorU8};

/// `None` for anything that isn't a plain RGB JPEG — imagery services serve
/// those, and a tile we can't read is a tile we skip the backdrop over.
pub(super) fn decode_jpeg(bytes: &[u8]) -> Option<Pixmap> {
    let mut decoder = Decoder::new(bytes);
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    if info.pixel_format != PixelFormat::RGB24 {
        return None;
    }

    let mut pixmap = Pixmap::new(u32::from(info.width), u32::from(info.height))?;
    for (target, rgb) in pixmap.pixels_mut().iter_mut().zip(pixels.chunks_exact(3)) {
        // Opaque, so premultiplying is the identity.
        *target = PremultipliedColorU8::from_rgba(rgb[0], rgb[1], rgb[2], 0xff)?;
    }
    Some(pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jpeg_encoder::{ColorType, Encoder};

    #[test]
    fn a_solid_tile_survives_the_round_trip() {
        let mut jpeg = Vec::new();
        Encoder::new(&mut jpeg, 100)
            .encode(&[0x20, 0x80, 0xc0].repeat(16 * 16), 16, 16, ColorType::Rgb)
            .unwrap();

        let pixmap = decode_jpeg(&jpeg).unwrap();

        assert_eq!(pixmap.width(), 16);
        let pixel = pixmap.pixels()[0];
        assert!(pixel.blue() > pixel.green() && pixel.green() > pixel.red());
    }

    #[test]
    fn garbage_is_not_a_tile() {
        assert!(decode_jpeg(b"not a jpeg").is_none());
    }
}
