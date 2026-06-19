use crate::dem::constants::MAX_DEM_TILE_SIDE;
use crate::geo::XyzTile;
use crate::matrix::{Raster, area_resample};
use crate::tree::{CachedChild, TileTreeError};

/// Stitch up to four z+1 children into a parent raster, then area-resample the
/// result back down to one tile-side. Same shape as
/// [`crate::dem::pyramid::build_parent_chunk`] for elevation, but each pixel
/// here is `channels` bytes (RGB or RGBA), so the resample runs per-channel.
///
/// For PMTiles imagery (`reads_intermediate_tiles == true`) every parent is
/// source-direct, so this path is statically unreached at runtime in v1. It
/// exists for trait completeness and for the future leaf-only imagery sources
/// (TIF RGB).
pub(crate) fn build_parent_raster(
    parent: XyzTile,
    children: &[CachedChild<Raster>],
) -> Result<Raster, TileTreeError> {
    if children.is_empty() {
        return Err(TileTreeError::MissingTile {
            z: parent.z,
            x: parent.x as u16,
            y: parent.y as u16,
        });
    }

    let (child_w, child_h, channels) = uniform_child_shape(children)?;
    let stitched_width = child_w * 2;
    let stitched_height = child_h * 2;
    let row_stride = child_w * channels;

    let child_x_pair = child_coord_pair(parent.x as u16);
    let child_y_pair = child_coord_pair(parent.y as u16);
    let cells: [Option<&[u8]>; 4] = std::array::from_fn(|idx| {
        let row_idx = idx / 2;
        let col_idx = idx % 2;
        find_child(children, child_x_pair[col_idx], child_y_pair[row_idx])
            .map(|child| child.raw.pixels.as_slice())
    });

    let mut pixels = Vec::with_capacity(stitched_width * stitched_height * channels);
    for row_idx in 0..2 {
        for y in 0..child_h {
            for col_idx in 0..2 {
                let row_start = y * row_stride;
                match cells[row_idx * 2 + col_idx] {
                    Some(cell) => {
                        pixels.extend_from_slice(&cell[row_start..row_start + row_stride])
                    }
                    None => pixels.extend(std::iter::repeat_n(0u8, row_stride)),
                }
            }
        }
    }

    let (width, height, pixels) =
        downscale_to_tile(stitched_width, stitched_height, channels, pixels);
    Ok(Raster {
        width,
        height,
        channels: channels as u8,
        pixels,
    })
}

fn uniform_child_shape(
    children: &[CachedChild<Raster>],
) -> Result<(usize, usize, usize), TileTreeError> {
    let first = children
        .first()
        .ok_or(TileTreeError::CorruptFile("parent has no children"))?;
    let width = usize::from(first.raw.width);
    let height = usize::from(first.raw.height);
    let channels = usize::from(first.raw.channels);
    for child in children {
        let cw = usize::from(child.raw.width);
        let ch = usize::from(child.raw.height);
        let cc = usize::from(child.raw.channels);
        if cw != width || ch != height || cc != channels {
            return Err(TileTreeError::CorruptFile(
                "WebP children have non-uniform shape; expected one (w, h, channels) per zoom",
            ));
        }
        if child.raw.pixels.len() != cw * ch * cc {
            return Err(TileTreeError::CorruptFile(
                "raw WebP raster pixel byte count does not match dimensions",
            ));
        }
    }

    Ok((width, height, channels))
}

fn child_coord_pair(parent_coord: u16) -> [u16; 2] {
    let first = parent_coord * 2;
    [first, first + 1]
}

fn find_child<'a>(
    children: &'a [CachedChild<Raster>],
    x: u16,
    y: u16,
) -> Option<&'a CachedChild<Raster>> {
    children
        .iter()
        .find(|child| child.tile.x as u16 == x && child.tile.y as u16 == y)
}

fn downscale_to_tile(
    source_width: usize,
    source_height: usize,
    channels: usize,
    pixels: Vec<u8>,
) -> (u16, u16, Vec<u8>) {
    let cap = usize::from(MAX_DEM_TILE_SIDE);
    let width = source_width.min(cap);
    let height = source_height.min(cap);
    if width == source_width && height == source_height {
        return (width as u16, height as u16, pixels);
    }

    let resampled_planes: Vec<Vec<u8>> = (0..channels)
        .map(|ch| {
            let plane: Vec<u8> = pixels.iter().skip(ch).step_by(channels).copied().collect();
            area_resample(&plane, source_width, source_height, width, height, |v| {
                f64::from(v)
            })
            .into_iter()
            .map(|v| v.round().clamp(0.0, 255.0) as u8)
            .collect()
        })
        .collect();

    let mut output = Vec::with_capacity(width * height * channels);
    for i in 0..(width * height) {
        for plane in &resampled_planes {
            output.push(plane[i]);
        }
    }
    (width as u16, height as u16, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stitch_children_preserves_quadrant_order() {
        let children = vec![
            cached_child(0, 0, 1, 1, 3, &[10, 20, 30]),
            cached_child(1, 0, 1, 1, 3, &[40, 50, 60]),
            cached_child(0, 1, 1, 1, 3, &[70, 80, 90]),
            cached_child(1, 1, 1, 1, 3, &[100, 110, 120]),
        ];

        let raster = build_parent_raster(tile(0, 0, 0), &children).unwrap();

        assert_eq!(raster.width, 2);
        assert_eq!(raster.height, 2);
        assert_eq!(raster.channels, 3);
        assert_eq!(
            raster.pixels,
            vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120],
        );
    }

    #[test]
    fn missing_quadrants_are_zero_filled() {
        let children = vec![cached_child(0, 0, 1, 1, 3, &[200, 100, 50])];

        let raster = build_parent_raster(tile(0, 0, 0), &children).unwrap();

        assert_eq!(raster.width, 2);
        assert_eq!(raster.height, 2);
        assert_eq!(raster.channels, 3);
        assert_eq!(raster.pixels, vec![200, 100, 50, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn non_uniform_children_are_rejected() {
        let children = vec![
            cached_child(0, 0, 1, 1, 3, &[1, 2, 3]),
            cached_child(1, 0, 1, 1, 3, &[4, 5, 6]),
            cached_child(0, 1, 2, 1, 3, &[7, 8, 9, 10, 11, 12]),
            cached_child(1, 1, 1, 1, 3, &[13, 14, 15]),
        ];

        let error = build_parent_raster(tile(0, 0, 0), &children).unwrap_err();
        assert!(matches!(error, TileTreeError::CorruptFile(_)));
    }

    #[test]
    fn rgba_round_trips_through_stitch() {
        let children = vec![
            cached_child(0, 0, 1, 1, 4, &[1, 2, 3, 255]),
            cached_child(1, 0, 1, 1, 4, &[4, 5, 6, 255]),
            cached_child(0, 1, 1, 1, 4, &[7, 8, 9, 255]),
            cached_child(1, 1, 1, 1, 4, &[10, 11, 12, 255]),
        ];

        let raster = build_parent_raster(tile(0, 0, 0), &children).unwrap();

        assert_eq!(raster.channels, 4);
        assert_eq!(raster.pixels.len(), 2 * 2 * 4);
        assert_eq!(&raster.pixels[..4], &[1, 2, 3, 255]);
        assert_eq!(&raster.pixels[4..8], &[4, 5, 6, 255]);
    }

    fn cached_child(
        x: u32,
        y: u32,
        width: u16,
        height: u16,
        channels: u8,
        pixels: &[u8],
    ) -> CachedChild<Raster> {
        CachedChild {
            tile: tile(1, x, y),
            raw: Raster {
                width,
                height,
                channels,
                pixels: pixels.to_vec(),
            },
        }
    }

    fn tile(z: u8, x: u32, y: u32) -> XyzTile {
        XyzTile { z, x, y }
    }
}
