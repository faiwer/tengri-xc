//! Block grid arithmetic for the compact tile-tree format.
//!
//! A block is a rectangle of up to 64×64 slots inside a single zoom's bounded
//! rect, anchored at that rect's top-left corner. Blocks are numbered
//! deepest-zoom-first to match [`crate::tree::slot_index::SlotIndex`]'s slot
//! order. The full block_id ↔ (z, block_x, block_y) mapping is pure arithmetic
//! — derivable from the file header alone, no on-disk addressing layer.

use crate::tree::bounds::XYZBounds;
use crate::tree::error::TileTreeError;
use crate::tree::format::{BLOCK_H, BLOCK_W};

/// Per-zoom block grid descriptor. `None` for zooms below `min_zoom`.
#[derive(Debug, Clone, Copy)]
pub struct ZoomBlocks {
    pub level_min_x: u16,
    pub level_min_y: u16,
    pub level_max_x: u16,
    pub level_max_y: u16,
    pub blocks_per_row: u32,
    pub blocks_per_col: u32,
    /// Block id of this zoom's first block. Deepest zoom starts at 0.
    pub block_id_offset: u64,
}

/// `block_x * 64`-sized rectangle inside zoom `z`. The last column / row may
/// be clipped — `dims.block_w` / `dims.block_h` carry the actual extent.
#[derive(Debug, Clone, Copy)]
pub struct BlockDescriptor {
    pub block_id: u64,
    pub zoom: u8,
    /// x-index of the block in the zoom's grid.
    pub block_x: u32,
    /// y-index of the block in the zoom's grid.
    pub block_y: u32,
    /// Dimensions of the block. 64x64 for all blocks but the last in a row or
    /// column.
    pub dims: BlockDimensions,
    /// Tile XYZ-coordinate of the block's top-left slot at this zoom.
    pub origin_x: u16,
    pub origin_y: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct BlockDimensions {
    pub block_w: u8,
    pub block_h: u8,
}

impl BlockDimensions {
    pub fn tile_count(self) -> u32 {
        u32::from(self.block_w) * u32::from(self.block_h)
    }
}

/// Locate the block containing a particular `(z, x, y)` tile.
#[derive(Debug, Clone, Copy)]
pub struct BlockLocation {
    pub block_id: u64,
    pub slot_in_block: u32,
}

#[derive(Debug)]
pub struct BlockGrid {
    /// Indexed by zoom (0..=max_zoom). `None` below `min_zoom`.
    zooms: Vec<Option<ZoomBlocks>>,
    /// All blocks in deterministic order: deepest-zoom-first, then row-major
    /// within each zoom. Indexed by `block_id`.
    blocks: Vec<BlockDescriptor>,
}

impl BlockGrid {
    pub fn new(bounds: XYZBounds, min_zoom: u8) -> Result<Self, TileTreeError> {
        let max_zoom = bounds.zoom;
        if min_zoom > max_zoom {
            return Err(TileTreeError::InvalidBounds("min_zoom exceeds max_zoom"));
        }

        let mut zooms: Vec<Option<ZoomBlocks>> = (0..=max_zoom).map(|_| None).collect();
        let mut blocks: Vec<BlockDescriptor> = Vec::new();
        let mut block_id: u64 = 0;
        for z in (min_zoom..=max_zoom).rev() {
            let descriptor = build_zoom_level(bounds, z, &mut blocks, &mut block_id)?;
            zooms[z as usize] = Some(descriptor);
        }
        Ok(Self { zooms, blocks })
    }

    pub fn total_blocks(&self) -> u64 {
        self.blocks.len() as u64
    }

    pub fn blocks(&self) -> &[BlockDescriptor] {
        &self.blocks
    }

    pub fn zoom(&self, z: u8) -> Result<&ZoomBlocks, TileTreeError> {
        self.zooms
            .get(z as usize)
            .and_then(|z_opt| z_opt.as_ref())
            .ok_or(TileTreeError::TileOutOfBounds { z, x: 0, y: 0 })
    }

    /// Pure-arithmetic block_id lookup for `(z, block_x, block_y)`. Returns
    /// `None` when the coordinates fall outside the bounded rect at zoom `z`
    /// (off the grid's right/bottom edges, or `z` is below `min_zoom`).
    pub fn block_id_at(&self, z: u8, block_x: u32, block_y: u32) -> Option<u64> {
        let zoom = self.zoom(z).ok()?;
        if block_x >= zoom.blocks_per_row || block_y >= zoom.blocks_per_col {
            return None;
        }
        Some(
            zoom.block_id_offset
                + u64::from(block_y) * u64::from(zoom.blocks_per_row)
                + u64::from(block_x),
        )
    }

    /// Locate the block containing `(z, x, y)`.
    pub fn block_for(&self, z: u8, x: u16, y: u16) -> Result<BlockLocation, TileTreeError> {
        let zoom = self.zoom(z)?;
        if x < zoom.level_min_x
            || x > zoom.level_max_x
            || y < zoom.level_min_y
            || y > zoom.level_max_y
        {
            return Err(TileTreeError::TileOutOfBounds { z, x, y });
        }
        let dx = u32::from(x - zoom.level_min_x);
        let dy = u32::from(y - zoom.level_min_y);
        let block_x = dx / u32::from(BLOCK_W);
        let block_y = dy / u32::from(BLOCK_H);
        let block_in_zoom =
            u64::from(block_y) * u64::from(zoom.blocks_per_row) + u64::from(block_x);
        let block_id = zoom.block_id_offset + block_in_zoom;
        let dims = clipped_dimensions(zoom, block_x, block_y);
        let slot_in_block_x = dx % u32::from(BLOCK_W);
        let slot_in_block_y = dy % u32::from(BLOCK_H);
        let slot_in_block = slot_in_block_y * u32::from(dims.block_w) + slot_in_block_x;
        Ok(BlockLocation {
            block_id,
            slot_in_block,
        })
    }
/// Build a single zoom's `ZoomBlocks` descriptor and append its block
/// descriptors (deepest-zoom-first, then row-major within the zoom) to
/// `blocks`. Bumps `next_block_id` past the last block written.
fn build_zoom_level(
    bounds: XYZBounds,
    z: u8,
    blocks: &mut Vec<BlockDescriptor>,
    next_block_id: &mut u64,
) -> Result<ZoomBlocks, TileTreeError> {
    let level = bounds.level_bounds(z)?;
    let level_w = level.x_tiles_count();
    let level_h = level.y_tiles_count();
    let blocks_per_row = u32::try_from(div_ceil_u64(level_w, u64::from(BLOCK_W)))
        .map_err(|_| TileTreeError::InvalidBounds("level too wide"))?;
    let blocks_per_col = u32::try_from(div_ceil_u64(level_h, u64::from(BLOCK_H)))
        .map_err(|_| TileTreeError::InvalidBounds("level too tall"))?;
    let descriptor = ZoomBlocks {
        level_min_x: level.min_x,
        level_min_y: level.min_y,
        level_max_x: level.max_x,
        level_max_y: level.max_y,
        blocks_per_row,
        blocks_per_col,
        block_id_offset: *next_block_id,
    };

    for block_y in 0..blocks_per_col {
        for block_x in 0..blocks_per_row {
            let dims = clipped_dimensions(&descriptor, block_x, block_y);
            let origin_x = level.min_x + (block_x as u16) * u16::from(BLOCK_W);
            let origin_y = level.min_y + (block_y as u16) * u16::from(BLOCK_H);
            blocks.push(BlockDescriptor {
                block_id: *next_block_id,
                zoom: z,
                block_x,
                block_y,
                dims,
                origin_x,
                origin_y,
            });
            *next_block_id += 1;
        }
    }

    Ok(descriptor)
}

/// Calculate the dimensions of a block that is clipped to the level bounds. It
/// is 64x64 for all blocks except the last block in a row or column, which is
/// clipped to the level bounds.
fn clipped_dimensions(zoom: &ZoomBlocks, block_x: u32, block_y: u32) -> BlockDimensions {
    let level_w = u64::from(zoom.level_max_x - zoom.level_min_x) + 1;
    let level_h = u64::from(zoom.level_max_y - zoom.level_min_y) + 1;
    let last_col = zoom.blocks_per_row.saturating_sub(1);
    let last_row = zoom.blocks_per_col.saturating_sub(1);
    let block_w = if block_x == last_col {
        (level_w - u64::from(block_x) * u64::from(BLOCK_W)) as u8
    } else {
        BLOCK_W
    };
    let block_h = if block_y == last_row {
        (level_h - u64::from(block_y) * u64::from(BLOCK_H)) as u8
    } else {
        BLOCK_H
    };
    BlockDimensions { block_w, block_h }
}

const fn div_ceil_u64(a: u64, b: u64) -> u64 {
    (a + b - 1) / b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_block_when_level_smaller_than_block() {
        let bounds = XYZBounds::new(2, 0, 0, 3, 3).unwrap();
        let grid = BlockGrid::new(bounds, 0).unwrap();
        // z=2 → 4×4 = 1 block (clipped to 4×4).
        // z=1 → 2×2 = 1 block (clipped to 2×2).
        // z=0 → 1×1 = 1 block (clipped to 1×1).
        assert_eq!(grid.total_blocks(), 3);
        assert_eq!(grid.zoom(2).unwrap().blocks_per_row, 1);
        assert_eq!(grid.zoom(2).unwrap().blocks_per_col, 1);
        assert_eq!(grid.blocks()[0].block_id, 0);
        assert_eq!(grid.blocks()[0].zoom, 2);
        assert_eq!(grid.blocks()[0].dims.block_w, 4);
    }

    #[test]
    fn multi_row_block_grid() {
        // Width 130 → 3 blocks per row (64 + 64 + 2). Height 70 → 2 blocks
        // per col (64 + 6).
        let bounds = XYZBounds::new(11, 0, 0, 129, 69).unwrap();
        let grid = BlockGrid::new(bounds, 11).unwrap();
        let z = grid.zoom(11).unwrap();
        assert_eq!(z.blocks_per_row, 3);
        assert_eq!(z.blocks_per_col, 2);
        assert_eq!(grid.total_blocks(), 6);

        let last = grid.blocks().last().unwrap();
        assert_eq!(last.dims.block_w, 2);
        assert_eq!(last.dims.block_h, 6);
    }

    #[test]
    fn block_for_locates_clipped_corner() {
        let bounds = XYZBounds::new(11, 0, 0, 129, 69).unwrap();
        let grid = BlockGrid::new(bounds, 11).unwrap();
        let loc = grid.block_for(11, 129, 69).unwrap();
        // The clipped corner block is the last in the grid.
        assert_eq!(loc.block_id, grid.total_blocks() - 1);
        // last slot in block (block_w=2, block_h=6): slot index = 5 * 2 + 1 = 11.
        assert_eq!(loc.slot_in_block, 11);
    }

    #[test]
    fn block_id_consistent_between_iter_and_lookup() {
        let bounds = XYZBounds::new(3, 0, 0, 7, 7).unwrap();
        let grid = BlockGrid::new(bounds, 0).unwrap();
        for descriptor in grid.blocks() {
            let loc = grid
                .block_for(descriptor.zoom, descriptor.origin_x, descriptor.origin_y)
                .unwrap();
            assert_eq!(loc.block_id, descriptor.block_id);
        }
    }

    #[test]
    fn children_of_root_at_max_zoom_is_empty() {
        let bounds = XYZBounds::new(2, 0, 0, 3, 3).unwrap();
        let grid = BlockGrid::new(bounds, 0).unwrap();
        let leaf = grid.blocks().iter().find(|b| b.zoom == 2).unwrap();
        assert!(grid.children_of(leaf).is_empty());
    }

    #[test]
    fn children_of_z0_returns_z1_block() {
        // Whole-world 3-zoom tree. z=0 has one tile, z=1 has 4, z=2 has 16.
        // Each parent's children fit in a single child block here.
        let bounds = XYZBounds::new(2, 0, 0, 3, 3).unwrap();
        let grid = BlockGrid::new(bounds, 0).unwrap();
        let root = grid.blocks().iter().find(|b| b.zoom == 0).unwrap();
        let kids = grid.children_of(root);
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].zoom, 1);
    }

    #[test]
    fn children_of_large_parent_block_returns_up_to_four() {
        // 64 wide × 64 tall parent at z=10 → 128 × 128 child tiles at z=11
        // → 4 child blocks (2 × 2).
        let bounds = XYZBounds::new(11, 0, 0, 127, 127).unwrap();
        let grid = BlockGrid::new(bounds, 10).unwrap();
        let parent = grid
            .blocks()
            .iter()
            .find(|b| b.zoom == 10 && b.block_x == 0 && b.block_y == 0)
            .unwrap();
        let kids = grid.children_of(parent);
        assert_eq!(kids.len(), 4);
        for kid in &kids {
            assert_eq!(kid.zoom, 11);
        }
    }
}
