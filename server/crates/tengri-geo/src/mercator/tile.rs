//! Slippy-map XYZ tiles, indexed off the Mercator plane.

use super::project::{MercatorRect, WORLD_SIZE_M};
use crate::Point;

/// `y` counts south from the top of the world, unlike Mercator metres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XyzTile {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

/// Metres a tile spans at `zoom`, on either axis.
pub fn tile_span_m(zoom: u8) -> f64 {
    WORLD_SIZE_M / f64::from(tiles_per_side(zoom))
}

/// North-west corner of `tile`, in Mercator metres.
pub fn tile_origin_m(tile: XyzTile) -> Point {
    let span = tile_span_m(tile.z);
    Point::new(
        f64::from(tile.x) * span - WORLD_SIZE_M / 2.0,
        WORLD_SIZE_M / 2.0 - f64::from(tile.y) * span,
    )
}

/// Every tile touching `rect`, row-major from the north-west corner.
pub fn tiles_covering(rect: MercatorRect, zoom: u8) -> Vec<XyzTile> {
    let max_index = f64::from(tiles_per_side(zoom) - 1);
    let span = tile_span_m(zoom);
    let tile_at = |distance_m: f64| (distance_m / span).floor().clamp(0.0, max_index) as u32;
    // A rect edge landing exactly on a tile seam belongs to the tile before it;
    // `ceil - 1` keeps that seam from pulling in a whole empty row or column.
    let tile_before =
        |distance_m: f64| ((distance_m / span).ceil() - 1.0).clamp(0.0, max_index) as u32;

    let first_x = tile_at(rect.min_x + WORLD_SIZE_M / 2.0);
    let last_x = tile_before(rect.max_x + WORLD_SIZE_M / 2.0).max(first_x);
    let first_y = tile_at(WORLD_SIZE_M / 2.0 - rect.max_y);
    let last_y = tile_before(WORLD_SIZE_M / 2.0 - rect.min_y).max(first_y);

    let mut tiles = Vec::new();
    for y in first_y..=last_y {
        for x in first_x..=last_x {
            tiles.push(XyzTile { z: zoom, x, y });
        }
    }
    tiles
}

fn tiles_per_side(zoom: u8) -> u32 {
    1u32 << zoom.min(31)
}

#[cfg(test)]
mod tests {
    use super::super::project::project_mercator_m;
    use super::*;
    use crate::PointE5;

    /// A point somewhere in the Chartreuse, inside the tile the ArcGIS sample
    /// URL asks for: `/tile/11/734/1058`.
    fn chartreuse() -> MercatorRect {
        let point = project_mercator_m(&PointE5::new(45_30000, 6_05000));
        MercatorRect::new(point.x, point.y, point.x, point.y)
    }

    #[test]
    fn a_point_lands_in_the_tile_that_serves_it() {
        let tiles = tiles_covering(chartreuse(), 11);

        assert_eq!(
            tiles,
            vec![XyzTile {
                z: 11,
                x: 1058,
                y: 734
            }]
        );
    }

    #[test]
    fn the_whole_world_is_one_tile_at_zoom_zero() {
        let world = MercatorRect::new(
            -WORLD_SIZE_M / 2.0,
            -WORLD_SIZE_M / 2.0,
            WORLD_SIZE_M / 2.0,
            WORLD_SIZE_M / 2.0,
        );

        assert_eq!(tiles_covering(world, 0), vec![XyzTile { z: 0, x: 0, y: 0 }]);
    }

    #[test]
    fn a_rect_spanning_a_seam_takes_both_tiles() {
        let span = tile_span_m(11);
        let seam = tile_origin_m(XyzTile {
            z: 11,
            x: 1058,
            y: 734,
        });
        let rect = MercatorRect::new(
            seam.x - 1.0,
            seam.y - span + 1.0,
            seam.x + 1.0,
            seam.y + 1.0,
        );

        let tiles = tiles_covering(rect, 11);

        assert_eq!(tiles.len(), 4);
        assert_eq!(
            tiles[0],
            XyzTile {
                z: 11,
                x: 1057,
                y: 733
            }
        );
        assert_eq!(
            tiles[3],
            XyzTile {
                z: 11,
                x: 1058,
                y: 734
            }
        );
    }

    #[test]
    fn the_origin_is_the_north_west_corner() {
        let tile = XyzTile {
            z: 11,
            x: 1058,
            y: 734,
        };
        let span = tile_span_m(11);

        let origin = tile_origin_m(tile);
        let inside = project_mercator_m(&PointE5::new(45_30000, 6_05000));

        assert!(inside.x >= origin.x && inside.x <= origin.x + span);
        assert!(inside.y <= origin.y && inside.y >= origin.y - span);
    }

    #[test]
    fn tiles_shrink_by_half_every_zoom() {
        assert_eq!(tile_span_m(0), WORLD_SIZE_M);
        assert!((tile_span_m(11) - WORLD_SIZE_M / 2048.0).abs() < 1e-9);
    }
}
