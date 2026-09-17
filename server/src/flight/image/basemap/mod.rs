//! The satellite mosaic drawn behind the track. Tiles are fetched per render
//! on a short budget: if they don't all arrive, the image falls back to the
//! white canvas rather than making the caller wait.

mod decode;
mod fetch;
mod mosaic;
mod tiles;

pub(super) use fetch::fetch;
pub use mosaic::Basemap;
pub(super) use mosaic::draw_basemap;
