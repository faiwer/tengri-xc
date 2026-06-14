use super::constants::MAX_DEM_TILE_SIDE;
use super::types::DemChunk;
use crate::geo::XyzTile;
use crate::matrix::area_resample;
use crate::tree::{CachedChild, TileTreeError as DemTreeError};

struct ChildTile<'a> {
    x: u16,
    y: u16,
    width: usize,
    height: usize,
    pixels: &'a [i16],
}

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

    let mut raw_children = Vec::with_capacity(children.len());
    for child in children {
        raw_children.push(child_tile(child)?);
    }
    stitch_children(parent, &raw_children)
}

fn child_tile(child: &CachedChild<DemChunk>) -> Result<ChildTile<'_>, DemTreeError> {
    let width = usize::from(child.raw.width);
    let height = usize::from(child.raw.height);
    if child.raw.pixels.len() != width * height {
        return Err(DemTreeError::CorruptFile(
            "raw DEM tile pixel count is wrong",
        ));
    }
    Ok(ChildTile {
        x: child.tile.x as u16,
        y: child.tile.y as u16,
        width,
        height,
        pixels: &child.raw.pixels,
    })
}

fn stitch_children(parent: XyzTile, children: &[ChildTile<'_>]) -> Result<DemChunk, DemTreeError> {
    let child_x_pair = child_coord_pair(parent.x as u16);
    let child_y_pair = child_coord_pair(parent.y as u16);
    let fallback_width = children
        .iter()
        .map(|child| child.width)
        .max()
        .ok_or(DemTreeError::CorruptFile("parent has no children"))?;
    let fallback_height = children
        .iter()
        .map(|child| child.height)
        .max()
        .ok_or(DemTreeError::CorruptFile("parent has no children"))?;
    let column_widths = child_x_pair.map(|lng| {
        children
            .iter()
            .filter(|child| child.x == lng)
            .map(|child| child.width)
            .max()
            .unwrap_or(fallback_width)
    });
    let row_heights = child_y_pair.map(|lat| {
        children
            .iter()
            .filter(|child| child.y == lat)
            .map(|child| child.height)
            .max()
            .unwrap_or(fallback_height)
    });
    let source_width = column_widths.iter().sum::<usize>();
    let source_height = row_heights.iter().sum::<usize>();

    // Resample each present child once to its (column_width, row_height) cell;
    // missing quadrants resample to a zero block of the same shape.
    let cells: [Vec<i16>; 4] = std::array::from_fn(|idx| {
        let row_idx = idx / 2;
        let col_idx = idx % 2;
        let cell_w = column_widths[col_idx];
        let cell_h = row_heights[row_idx];
        match find_child(children, child_x_pair[col_idx], child_y_pair[row_idx]) {
            Some(child) => resample_child(child, cell_w, cell_h),
            // Might be missing because the child is out of the source bounds.
            None => vec![0; cell_w * cell_h],
        }
    });

    let mut pixels = Vec::with_capacity(source_width * source_height);
    for (row_idx, _lat) in child_y_pair.iter().enumerate() {
        for y in 0..row_heights[row_idx] {
            for (col_idx, _lng) in child_x_pair.iter().enumerate() {
                let cell = &cells[row_idx * 2 + col_idx];
                let width = column_widths[col_idx];
                let row_start = y * width;
                pixels.extend_from_slice(&cell[row_start..row_start + width]);
            }
        }
    }

    let (width, height, pixels) = downscale_to_dem_tile(source_width, source_height, pixels);
    Ok(DemChunk {
        width,
        height,
        pixels,
    })
}

/// If parent's X is 30, its children's Xs are 60 and 61.
fn child_coord_pair(parent_coord: u16) -> [u16; 2] {
    let first = parent_coord * 2;
    [first, first + 1]
}

fn find_child<'a>(children: &'a [ChildTile<'a>], lng: u16, lat: u16) -> Option<&'a ChildTile<'a>> {
    children
        .iter()
        .find(|child| child.x == lng && child.y == lat)
}

fn resample_child(child: &ChildTile<'_>, target_width: usize, target_height: usize) -> Vec<i16> {
    if child.width == target_width && child.height == target_height {
        return child.pixels.to_vec();
    }

    area_resample(
        child.pixels,
        child.width,
        child.height,
        target_width,
        target_height,
        |value| f64::from(value.max(0)),
    )
    .into_iter()
    .map(|value| value.round() as i16)
    .collect()
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
    fn variable_child_widths_are_resampled_not_zero_padded() {
        let children = vec![
            cached_child(0, 0, 1, 1, &[7]),
            cached_child(1, 0, 1, 1, &[3]),
            cached_child(0, 1, 2, 1, &[8, 10]),
            cached_child(1, 1, 1, 1, &[4]),
        ];

        let chunk = build_parent_chunk(tile(0, 0, 0), &children).unwrap();

        assert_eq!(chunk.width, 3);
        assert_eq!(chunk.height, 2);
        assert_eq!(chunk.pixels, vec![7, 7, 3, 8, 10, 4]);
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
