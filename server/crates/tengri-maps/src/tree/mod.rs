//! Single-file tile tree container.

mod bounds;
mod builder;
mod error;
mod export;
mod format;
mod index;
mod metadata;
mod reader;
mod slot_index;
mod writer;

pub use bounds::{MAX_WEB_MERCATOR_TREE_ZOOM, XYZBounds};
pub use error::TileTreeError;
pub use export::{CachedChild, TileTreeExportAdapter, TileTreeExportReport, TileTreeExporter};
pub use metadata::TileKind;
pub use reader::TileTreeReader;
pub use slot_index::SlotIndex;
