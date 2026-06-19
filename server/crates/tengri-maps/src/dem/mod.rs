//! Digital elevation model utilities.

mod bitpack;
mod compress;
pub(crate) mod constants;
mod decompress;
mod dem_export_adapter;
mod error;
mod pyramid;
mod resolution;
pub(crate) mod serve;
mod tile_file;
mod tree_export;
mod types;

#[cfg(test)]
mod tests;

pub use constants::DEM_QUANTIZATION_METERS;
pub use error::DemError;
pub use tree_export::{DemTree, DemTreeBuilder, DemTreeExportReport};
pub use types::DemChunk;
