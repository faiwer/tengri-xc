use std::io::{BufReader, Cursor};

use image_webp::WebPDecoder;

use crate::tree::TileTreeError;

/// Header facts the WebP exporter needs to decide whether passthrough is
/// reachable: dim must already match the archive's tile-side, and the source
/// must be RGB (alpha-shipping sources fall through to the matrix path so it
/// can strip alpha).
pub(crate) struct WebpHeader {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
}

/// Read the WebP header just far enough to recover dims + alpha presence. Cheap
/// (~30 bytes parsed), no pixel decode. Used by
/// [`crate::webp::WebpExportAdapter`] to validate that passthrough source bytes
/// meet the archive's contract before copying them verbatim.
pub(crate) fn peek_webp_header(bytes: &[u8]) -> Result<WebpHeader, TileTreeError> {
    let decoder = WebPDecoder::new(BufReader::new(Cursor::new(bytes)))?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 {
        return Err(TileTreeError::CorruptFile("WebP has zero dimensions"));
    }

    if width > u32::from(u16::MAX) || height > u32::from(u16::MAX) {
        return Err(TileTreeError::CorruptFile(
            "WebP dimensions exceed u16::MAX; source is not a tile",
        ));
    }

    Ok(WebpHeader {
        width,
        height,
        has_alpha: decoder.has_alpha(),
    })
}
