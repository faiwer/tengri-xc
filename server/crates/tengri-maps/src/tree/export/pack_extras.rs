//! End-pass that packs neighbour self-payloads into each block envelope's
//! leftover headroom. Walks every block, computes packing decisions from
//! the in-RAM `len_self` table alone, and only reads bytes from disk after
//! the chosen set is known to fit. Blocks where nothing ends up packed
//! (`mode_byte` stays 0) are skipped — DFS already wrote a mode-0 envelope
//! there before, so a rewrite would just reproduce the same bytes.
//!
//! Per block the worst case is `1 self read + 7 extra reads + 1 envelope write
//! = 9 × 16 KiB`.

use std::fs::File;
use std::os::unix::fs::FileExt;

use crate::tree::blocks::{BlockDescriptor, BlockGrid};
use crate::tree::error::TileTreeError;
use crate::tree::format::{BLOCK_SIZE, ENVELOPE_OVERHEAD, EXTRA_PREFIX_LEN, HEADER_LEN};

use super::encode::{build_envelope_raw, mode};

/// Walks every block, computes packing decisions from the in-RAM `len_self`
/// table alone, and only reads bytes from disk after the chosen set is known to
/// fit. Blocks whose initial envelope already has no extras (`mode == 0`) are
/// skipped — the orchestrator wrote them correctly during DFS.
pub(super) fn run(dest: &File, grid: &BlockGrid, len_self: &[u16]) -> Result<(), TileTreeError> {
    for block in grid.blocks() {
        pack_one(dest, grid, len_self, block)?;
    }
    Ok(())
}

/// Packs the extras for a single index block.
fn pack_one(
    dest: &File,
    grid: &BlockGrid,
    len_self: &[u16],
    block: &BlockDescriptor,
) -> Result<(), TileTreeError> {
    let block_idx = usize::try_from(block.block_id)
        .map_err(|_| TileTreeError::InvalidBounds("block id exceeds usize"))?;
    let self_len = u64::from(len_self[block_idx]);
    let mut headroom = BLOCK_SIZE - ENVELOPE_OVERHEAD - self_len;

    let candidates = collect_candidates(block, grid);
    let mut mode_byte: u8 = 0;
    let mut chosen: Vec<u64> = Vec::with_capacity(7);
    for (bit, neighbour_id) in candidates {
        let Some(id) = neighbour_id else { continue };
        let neighbour_idx = usize::try_from(id)
            .map_err(|_| TileTreeError::InvalidBounds("neighbour block id exceeds usize"))?;
        let neighbour_len = u64::from(len_self[neighbour_idx]);
        if neighbour_len == 0 {
            continue;
        }

        let cost = EXTRA_PREFIX_LEN + neighbour_len;
        if headroom >= cost {
            mode_byte |= bit;
            headroom -= cost;
            chosen.push(id);
        }
    }
    if mode_byte == 0 {
        return Ok(());
    }

    let envelope_offset = HEADER_LEN + block.block_id * BLOCK_SIZE;
    let self_payload = read_self_payload(dest, envelope_offset, self_len as usize)?;

    let mut extras_owned: Vec<Vec<u8>> = Vec::with_capacity(chosen.len());
    for id in &chosen {
        let extra_idx = usize::try_from(*id)
            .map_err(|_| TileTreeError::InvalidBounds("neighbour block id exceeds usize"))?;
        let extra_len = len_self[extra_idx] as usize;
        let extra_offset = HEADER_LEN + *id * BLOCK_SIZE;
        let payload = read_self_payload(dest, extra_offset, extra_len)?;
        extras_owned.push(payload);
    }

    let extras: Vec<&[u8]> = extras_owned.iter().map(|v| v.as_slice()).collect();
    let new_envelope = build_envelope_raw(mode_byte, &self_payload, &extras)?;
    dest.write_all_at(&new_envelope, envelope_offset)?;
    Ok(())
}

fn read_self_payload(
    dest: &File,
    envelope_offset: u64,
    self_len: usize,
) -> Result<Vec<u8>, TileTreeError> {
    let mut buf = vec![0u8; (ENVELOPE_OVERHEAD as usize) + self_len];
    dest.read_exact_at(&mut buf, envelope_offset)?;
    Ok(buf[ENVELOPE_OVERHEAD as usize..].to_vec())
}

fn collect_candidates(block: &BlockDescriptor, grid: &BlockGrid) -> Vec<(u8, Option<u64>)> {
    let mut out: Vec<(u8, Option<u64>)> = Vec::with_capacity(7);
    let z = block.zoom;
    let parent_zoom = z.checked_sub(1);
    let parent_coords = parent_zoom.map(|pz| (pz, block.block_x / 2, block.block_y / 2));
    let parent_id = parent_coords.and_then(|(pz, px, py)| grid.block_id_at(pz, px, py));
    out.push((mode::PARENT, parent_id));

    for (bit, kind) in [
        (mode::SIBLING_HORIZONTAL, SiblingKind::Horizontal),
        (mode::SIBLING_VERTICAL, SiblingKind::Vertical),
        (mode::SIBLING_DIAGONAL, SiblingKind::Diagonal),
    ] {
        let coord = sibling_coords(block.block_x, block.block_y, kind);
        let id = coord.and_then(|(bx, by)| grid.block_id_at(z, bx, by));
        out.push((bit, id));
    }

    if let (Some(pz), Some((_, parent_block_x, parent_block_y))) = (parent_zoom, parent_coords) {
        for (bit, kind) in [
            (mode::COUSIN_HORIZONTAL, SiblingKind::Horizontal),
            (mode::COUSIN_VERTICAL, SiblingKind::Vertical),
            (mode::COUSIN_DIAGONAL, SiblingKind::Diagonal),
        ] {
            let coord = sibling_coords(parent_block_x, parent_block_y, kind);
            let id = coord.and_then(|(bx, by)| grid.block_id_at(pz, bx, by));
            out.push((bit, id));
        }
    } else {
        for bit in [
            mode::COUSIN_HORIZONTAL,
            mode::COUSIN_VERTICAL,
            mode::COUSIN_DIAGONAL,
        ] {
            out.push((bit, None));
        }
    }
    out
}

#[derive(Clone, Copy)]
enum SiblingKind {
    Horizontal,
    Vertical,
    Diagonal,
}

fn sibling_coords(block_x: u32, block_y: u32, kind: SiblingKind) -> Option<(u32, u32)> {
    let dx_sign: i32 = if block_x % 2 == 0 { 1 } else { -1 };
    let dy_sign: i32 = if block_y % 2 == 0 { 1 } else { -1 };
    let (dx, dy) = match kind {
        SiblingKind::Horizontal => (dx_sign, 0),
        SiblingKind::Vertical => (0, dy_sign),
        SiblingKind::Diagonal => (dx_sign, dy_sign),
    };
    let nx = i64::from(block_x) + i64::from(dx);
    let ny = i64::from(block_y) + i64::from(dy);
    if nx < 0 || ny < 0 {
        return None;
    }
    Some((nx as u32, ny as u32))
}
