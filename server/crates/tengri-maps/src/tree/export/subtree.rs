use crate::geo::XyzTile;
use crate::tree::{TileTreeError, XYZBounds};

use super::super::writer::TileTreeFile;
use super::adapter::{CachedChild, TileTreeExportAdapter};

/// Recursively export the subtree starting from the given tile. It goes
/// top-down, from the parent tile to the leaves. It returns the raw tile if
/// needed to compute the parent tiles.
pub(super) fn export_subtree<A: TileTreeExportAdapter>(
    adapter: &A,
    reader: &mut A::Reader,
    bounds: XYZBounds,
    tile: XyzTile,
    // Whether we need the raw tile to compute the parent tiles.
    raw_needed: bool,
    emit: &mut impl FnMut(XyzTile, Vec<u8>) -> Result<(), TileTreeError>,
) -> Result<Option<A::SourceTile>, TileTreeError> {
    if let Some(raw) = adapter.try_read_source_tile(reader, tile)? {
        // The source support intermediate tiles, so we can write the payload
        // to the tree file right away.
        let payload = adapter.encode_payload(&raw)?;
        emit(tile, payload)?;

        if raw_needed {
            export_children_write_only(adapter, reader, bounds, tile, emit)?;
            return Ok(Some(raw));
        }

        drop(raw); // Avoid memory bomb.
        // Recursively export the children subtrees.
        export_children_write_only(adapter, reader, bounds, tile, emit)?;
        return Ok(None);
    }

    if tile.z == bounds.zoom {
        // The tile is not present in the source. The source is broken.
        return Err(TileTreeError::MissingTile {
            z: tile.z,
            x: tile.x as u16,
            y: tile.y as u16,
        });
    }

    let mut children = Vec::new();
    for child in bounded_child_tiles(bounds, tile) {
        let raw = export_subtree(
            adapter, reader, bounds, child,
            // Request the raw tile to compute the parent tiles.
            true, emit,
        )?
        .ok_or(TileTreeError::CorruptFile("child did not return raw tile"))?;
        children.push(CachedChild { tile: child, raw });
    }

    let raw = adapter.reduce_children_to_tile(tile, &children)?;
    drop(children); // Avoid memory bomb.
    let payload = adapter.encode_payload(&raw)?;
    emit(tile, payload)?;
    if raw_needed {
        Ok(Some(raw))
    } else {
        drop(raw);
        Ok(None)
    }
}

fn export_children_write_only<A: TileTreeExportAdapter>(
    adapter: &A,
    reader: &mut A::Reader,
    bounds: XYZBounds,
    tile: XyzTile,
    emit: &mut impl FnMut(XyzTile, Vec<u8>) -> Result<(), TileTreeError>,
) -> Result<(), TileTreeError> {
    if tile.z == bounds.zoom {
        // No children to export.
        return Ok(());
    }

    for child in bounded_child_tiles(bounds, tile) {
        export_subtree(adapter, reader, bounds, child, false, emit)?;
    }
    Ok(())
}

pub(super) fn add_payload(
    tree: &mut TileTreeFile,
    tile: XyzTile,
    payload: &[u8],
) -> Result<(), TileTreeError> {
    tree.add(tile.z, tile.x as u16, tile.y as u16, payload)?;
    Ok(())
}

/// Returns the child tiles of the parent tile (zoom level +1) that are within
/// the bounds.
pub(super) fn bounded_child_tiles(
    bounds: XYZBounds,
    parent: XyzTile,
) -> impl Iterator<Item = XyzTile> {
    let z = parent.z + 1;
    child_tiles(parent)
        .into_iter()
        .filter(move |tile| bounds.contains(z, tile.x as u16, tile.y as u16))
}

/// Returns the 4 child tiles of the parent tile (zoom level +1).
fn child_tiles(parent: XyzTile) -> [XyzTile; 4] {
    let z = parent.z + 1;
    let first_x = parent.x * 2;
    let first_y = parent.y * 2;
    [
        XyzTile {
            z,
            x: first_x,
            y: first_y,
        },
        XyzTile {
            z,
            x: first_x + 1,
            y: first_y,
        },
        XyzTile {
            z,
            x: first_x,
            y: first_y + 1,
        },
        XyzTile {
            z,
            x: first_x + 1,
            y: first_y + 1,
        },
    ]
}
