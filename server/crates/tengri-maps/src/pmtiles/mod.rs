mod dem_source;
mod terrarium;
mod dem_source_reader;
mod constants;

pub use dem_source::PmtilesDemSource;

impl From<::pmtiles::PmtError> for crate::tree::TileTreeError {
    fn from(error: ::pmtiles::PmtError) -> Self {
        crate::tree::TileTreeError::external(error)
    }
}

impl From<image_webp::DecodingError> for crate::tree::TileTreeError {
    fn from(error: image_webp::DecodingError) -> Self {
        crate::tree::TileTreeError::external(error)
    }
}
