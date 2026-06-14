//! Digital elevation model utilities.

mod bitpack;
mod compress;
pub(crate) mod constants;
mod decompress;
mod error;
mod progress;
mod pyramid;
mod resolution;
pub(crate) mod serve;
mod source;
mod tile_file;
mod tree_export;
mod types;

#[cfg(test)]
mod tests;

pub use source::{DemSource, DemSourceReader};
pub use tree_export::{DemTree, DemTreeBuilder, DemTreeExportReport};
pub use types::{DemChunk, DemPixelMatrix};
