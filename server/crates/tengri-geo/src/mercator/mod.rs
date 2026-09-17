//! Web Mercator projection and the tile grid cut from it. `tengri-maps` has
//! equivalent math over lat/lon degrees (`geo/mercator.rs`, `geo/xyz.rs`);
//! this one works in metres, which is what a canvas layout needs.

mod project;
mod tile;

pub use project::{MercatorRect, WORLD_SIZE_M, mercator_bounds, project_points_mercator_m};
pub use tile::{XyzTile, tile_origin_m, tile_span_m, tiles_covering};
