//! WebP raster tile export pipeline. Mirrors [`crate::dem`] for the
//! satellite/imagery use case: the on-disk archive carries opaque WebP payloads
//! and the builder ingests an arbitrary [`crate::tree::TileSource<Tile =
//! Raster>`](crate::tree::TileSource).

pub(crate) mod decode;
mod encode;
pub(crate) mod peek;
mod pyramid;
pub(crate) mod serve;
mod tree_export;
mod webp_export_adapter;

pub use tree_export::{WebpTree, WebpTreeBuilder, WebpTreeExportReport};
