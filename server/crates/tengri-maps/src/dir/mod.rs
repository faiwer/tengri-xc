//! Loose-tile imagery source: reads `<root>/<z>/<x>/<prefix><y>.<ext>` files
//! from disk and serves them to the [`crate::webp::WebpTree`] exporter. Sibling
//! of [`crate::pmtiles`] but unpacked — a directory of raw `.webp`/`.png`/`.jpg`
//! tiles rather than a single PMTiles archive.

mod decode;
mod imagery_source;

pub use imagery_source::DirImagerySource;

impl From<::png::DecodingError> for crate::tree::TileTreeError {
    fn from(error: ::png::DecodingError) -> Self {
        crate::tree::TileTreeError::external(error)
    }
}

impl From<::jpeg_decoder::Error> for crate::tree::TileTreeError {
    fn from(error: ::jpeg_decoder::Error) -> Self {
        crate::tree::TileTreeError::external(error)
    }
}
