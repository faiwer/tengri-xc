use anyhow::Context;
use jpeg_encoder::{ColorType, Encoder};
use tiny_skia::Pixmap;

const QUALITY: u8 = 85;

/// The canvas is opaque, so premultiplied RGBA is already straight RGB and the
/// alpha byte can simply be dropped.
pub(super) fn encode(pixmap: &Pixmap) -> anyhow::Result<Vec<u8>> {
    // `Pixmap::data` is a flat buffer of 4 bytes per pixel (RGBA); keep the
    // first three of each.
    let mut rgb = Vec::with_capacity(pixmap.data().len() / 4 * 3);
    for pixel in pixmap.data().chunks_exact(4) {
        rgb.extend_from_slice(&pixel[..3]);
    }

    let mut out = Vec::new();
    Encoder::new(&mut out, QUALITY)
        .encode(
            &rgb,
            pixmap.width() as u16,
            pixmap.height() as u16,
            ColorType::Rgb,
        )
        .context("encoding track image")?;
    Ok(out)
}
