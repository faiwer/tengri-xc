use crate::geo::XyzTile;
use crate::tree::{TileTreeError, XYZBounds};

use super::adapter::{CachedChild, TileTreeExportAdapter};
use super::cache::RawTileCache;
use super::subtree::bounded_child_tiles;

/// Recursively compute the top part of the tree, the part that is above the
/// split zoom branches. The bottom part must be ready at this point. The top
/// part of each must be cached for reduction.
/// 
/// It returns the raw tile, instead of the compressed payload, because it'll
/// be used to compute the parent tile.
pub(super) fn reduce_cached<A: TileTreeExportAdapter>(
    adapter: &A,
    cache: &RawTileCache,
    bounds: XYZBounds,
    tile: XyzTile,
    // The zoom level at which the tree should have been split into subtrees.
    split_zoom: u8,
    // Callback to emit the payload of the tile.
    emit: &mut impl FnMut(XyzTile, Vec<u8>) -> Result<(), TileTreeError>,
) -> Result<A::SourceTile, TileTreeError> {
    if let Some(raw) = cache.consume(adapter, tile)? {
        return Ok(raw);
    }

    if tile.z >= split_zoom {
        // The tiles below the split zoom level are not ready yet :(
        return Err(TileTreeError::MissingTile {
            z: tile.z,
            x: tile.x as u16,
            y: tile.y as u16,
        });
    }

    let mut children = Vec::new();
    for child in bounded_child_tiles(bounds, tile) {
        let raw = reduce_cached(adapter, cache, bounds, child, split_zoom, emit)?;
        children.push(CachedChild { tile: child, raw });
    }

    let raw = adapter.reduce_children_to_tile(tile, &children)?;
    drop(children);
    let payload = adapter.encode_payload(&raw)?;
    emit(tile, payload)?;
    Ok(raw)
}
