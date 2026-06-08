//! Digital elevation model utilities.

mod bitpack;
mod compress;
mod decompress;
mod error;
mod export;
mod metadata;
mod tile_file;
pub mod types;
mod xyz_tiles;

pub use compress::compress_tile;
pub use crate::constants::{DEM_QUANTIZATION_METERS, MAX_DEM_TILE_SIDE};
pub use decompress::decompress_tile;
pub use error::DemError;
pub use export::{LeafTileExportOptions, LeafTileExportReport, export_leaf_tiles};
pub use metadata::{
    DemTileSetMetadata, TILESET_METADATA_FILE, read_tileset_metadata, write_tileset_metadata,
};
pub use tile_file::{read_tile, write_tile};
pub use types::{CompressedDemTile, Fix, UncompressedDemTile};
pub use xyz_tiles::{
    WEB_MERCATOR_MAX_LAT, XyzTile, XyzTileError, xyz_tile_bounds, xyz_tiles_for_bounds,
};
