use super::constants::MAX_DEM_TILE_SIDE;
use super::types::DemChunk;
use crate::geo::XyzTile;
use crate::matrix::area_resample;
use crate::tree::{CachedChild, TileTreeError as DemTreeError};

/// Reduce a parent tile from its (up to four) z+1 children. Children are
/// produced by the same orchestrator pass and therefore share a single `(width,
/// height)` shape — sourced from `cap_dem_matrix` for leaves or from this same
/// function for deeper parents — so the stitch is a plain 2×2 layout, no
/// per-cell resample. Missing quadrants (out-of-source-bounds) fill with zeros.
pub(crate) fn build_parent_chunk(
    parent: XyzTile,
    children: &[CachedChild<DemChunk>],
) -> Result<DemChunk, DemTreeError> {
    if children.is_empty() {
        return Err(DemTreeError::MissingTile {
            z: parent.z,
            x: parent.x as u16,
            y: parent.y as u16,
        });
    }
    let (child_w, child_h) = uniform_child_shape(children)?;
    let stitched_width = child_w * 2;
    let stitched_height = child_h * 2;

    let child_x_pair = child_coord_pair(parent.x as u16);
    let child_y_pair = child_coord_pair(parent.y as u16);
    let cells: [Option<&[i16]>; 4] = std::array::from_fn(|idx| {
        let row_idx = idx / 2;
        let col_idx = idx % 2;
        find_child(children, child_x_pair[col_idx], child_y_pair[row_idx])
            .map(|child| child.raw.pixels.as_slice())
    });

    let mut pixels = Vec::with_capacity(stitched_width * stitched_height);
    for row_idx in 0..2 {
        for y in 0..child_h {
            for col_idx in 0..2 {
                let row_start = y * child_w;
                match cells[row_idx * 2 + col_idx] {
                    Some(cell) => pixels.extend_from_slice(&cell[row_start..row_start + child_w]),
                    None => pixels.extend(std::iter::repeat_n(0i16, child_w)),
                }
            }
        }
    }

    let (width, height, pixels) = downscale_to_dem_tile(stitched_width, stitched_height, pixels);
    Ok(DemChunk {
        width,
        height,
        pixels,
    })
}

fn uniform_child_shape(children: &[CachedChild<DemChunk>]) -> Result<(usize, usize), DemTreeError> {
    let first = children
        .first()
        .ok_or(DemTreeError::CorruptFile("parent has no children"))?;
    let width = usize::from(first.raw.width);
    let height = usize::from(first.raw.height);
    for child in children {
        let cw = usize::from(child.raw.width);
        let ch = usize::from(child.raw.height);
        if cw != width || ch != height {
            return Err(DemTreeError::CorruptFile(
                "DEM children have non-uniform shape; expected a single (w, h) per zoom",
            ));
        }
        if child.raw.pixels.len() != cw * ch {
            return Err(DemTreeError::CorruptFile(
                "raw DEM tile pixel count is wrong",
            ));
        }
    }
    Ok((width, height))
}

/// If parent's X is 30, its children's Xs are 60 and 61.
fn child_coord_pair(parent_coord: u16) -> [u16; 2] {
    let first = parent_coord * 2;
    [first, first + 1]
}

fn find_child<'a>(
    children: &'a [CachedChild<DemChunk>],
    x: u16,
    y: u16,
) -> Option<&'a CachedChild<DemChunk>> {
    children
        .iter()
        .find(|child| child.tile.x as u16 == x && child.tile.y as u16 == y)
}

fn downscale_to_dem_tile(
    source_width: usize,
    source_height: usize,
    pixels: Vec<i16>,
) -> (u16, u16, Vec<i16>) {
    let width = source_width.min(usize::from(MAX_DEM_TILE_SIDE));
    let height = source_height.min(usize::from(MAX_DEM_TILE_SIDE));
    if width == source_width && height == source_height {
        return (width as u16, height as u16, pixels);
    }

    let pixels = area_resample(
        &pixels,
        source_width,
        source_height,
        width,
        height,
        |value| f64::from(value.max(0)),
    )
    .into_iter()
    .map(|value| value.round() as i16)
    .collect();
    (width as u16, height as u16, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stitch_children_preserves_quadrant_order() {
        let children = vec![
            cached_child(0, 0, 1, 1, &[1]),
            cached_child(1, 0, 1, 1, &[2]),
            cached_child(0, 1, 1, 1, &[3]),
            cached_child(1, 1, 1, 1, &[4]),
        ];

        let chunk = build_parent_chunk(tile(0, 0, 0), &children).unwrap();

        assert_eq!(chunk.width, 2);
        assert_eq!(chunk.height, 2);
        assert_eq!(chunk.pixels, vec![1, 2, 3, 4]);
    }

    #[test]
    fn missing_quadrants_are_zero_filled() {
        let children = vec![cached_child(0, 0, 1, 1, &[7])];

        let chunk = build_parent_chunk(tile(0, 0, 0), &children).unwrap();

        assert_eq!(chunk.width, 2);
        assert_eq!(chunk.height, 2);
        assert_eq!(chunk.pixels, vec![7, 0, 0, 0]);
    }

    #[test]
    fn non_uniform_children_are_rejected() {
        let children = vec![
            cached_child(0, 0, 1, 1, &[7]),
            cached_child(1, 0, 1, 1, &[3]),
            cached_child(0, 1, 2, 1, &[8, 10]),
            cached_child(1, 1, 1, 1, &[4]),
        ];

        let error = build_parent_chunk(tile(0, 0, 0), &children).unwrap_err();
        assert!(matches!(error, DemTreeError::CorruptFile(_)));
    }

    fn cached_child(
        lng: u32,
        lat: u32,
        width: u16,
        height: u16,
        pixels: &[i16],
    ) -> CachedChild<DemChunk> {
        CachedChild {
            tile: tile(1, lng, lat),
            raw: DemChunk::from_i16(width, height, pixels.to_vec()),
        }
    }

    fn tile(z: u8, x: u32, y: u32) -> XyzTile {
        XyzTile { z, x, y }
    }
}
