use crate::serve::{ServedTile, TileServeError, TileServeFormat};

/// Serve format for `.tengri-map` archives. Stored payloads are already WebP
/// byte streams (either passthrough'd from the source PMTiles or freshly
/// encoded by libwebp at build time), so rendering is a verbatim pass-through
/// with the right `image/webp` content type.
pub(crate) struct WebpServeFormat;

impl TileServeFormat for WebpServeFormat {
    fn route_prefix(&self) -> &'static str {
        "/imagery/"
    }

    fn file_extension(&self) -> &'static str {
        ".webp"
    }

    fn render(&self, payload: &[u8]) -> Result<ServedTile, TileServeError> {
        Ok(ServedTile {
            content_type: "image/webp",
            body: payload.to_vec(),
        })
    }
}
