//! Single-file tile tree container.

mod blocks;
mod bounds;
mod error;
mod export;
mod format;
mod metadata;
mod reader;
mod size_stream;
mod slot_index;

pub use bounds::{MAX_WEB_MERCATOR_TREE_ZOOM, XYZBounds};
pub use error::TileTreeError;
pub use export::{CachedChild, TileTreeExportAdapter, TileTreeExportReport, TileTreeExporter};
pub use metadata::TileKind;
pub use reader::TileTreeReader;
pub use slot_index::SlotIndex;
