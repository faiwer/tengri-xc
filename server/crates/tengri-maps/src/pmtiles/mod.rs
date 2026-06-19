mod dem_source;
mod terrarium;
mod dem_source_reader;
mod constants;
mod imagery_source;

pub use dem_source::PmtilesDemSource;
pub use imagery_source::PmtilesImagerySource;

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
