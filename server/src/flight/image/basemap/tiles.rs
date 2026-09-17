//! Which tiles, at which zoom, cover the canvas.

use tengri_geo::{Point, WORLD_SIZE_M, XyzTile, tile_origin_m, tile_span_m, tiles_covering};

use crate::flight::image::layout::Layout;

/// The deepest zoom the imagery service serves usefully. Only flights under
/// ~1.5 km across ask for more.
const MAX_ZOOM: u8 = 15;

/// A guard, not a budget: flooring the zoom makes a tile cover at least
/// `tile_size` canvas pixels, so with the usual 256 px tiles a 500 px canvas
/// crosses at most one seam per axis — four tiles, nine with room to spare. A
/// service cutting tiles much smaller than that would blow past this and lose
/// its backdrop, which beats hammering it with dozens of requests.
const MAX_TILES: usize = 9;

pub(super) struct TilePlan {
    pub tiles: Vec<XyzTile>,
    pub columns: u32,
    pub rows: u32,
    /// North-west corner of the whole mosaic, in Mercator metres.
    pub origin: Point,
    /// Metres one mosaic pixel covers.
    pub metres_per_px: f64,
    /// Pixel side the service's tiles are expected to have.
    pub tile_size: u32,
}

pub(super) fn plan(layout: &Layout, tile_size: u32) -> Option<TilePlan> {
    let scale = layout.scale();
    if !scale.is_finite() || scale <= 0.0 || tile_size == 0 {
        return None;
    }
    let bounds = layout.covered_bounds();

    // Flooring picks the zoom whose pixels are *coarser* than the canvas', so
    // the mosaic is upscaled by up to 2× — invisible behind the track, and it
    // keeps the tile count down.
    let zoom = (scale * WORLD_SIZE_M / f64::from(tile_size))
        .log2()
        .floor()
        .clamp(0.0, f64::from(MAX_ZOOM)) as u8;

    let tiles = tiles_covering(bounds, zoom);
    if tiles.is_empty() || tiles.len() > MAX_TILES {
        return None;
    }

    let first = *tiles.first()?;
    let last = *tiles.last()?;
    Some(TilePlan {
        columns: last.x - first.x + 1,
        rows: last.y - first.y + 1,
        origin: tile_origin_m(first),
        metres_per_px: tile_span_m(zoom) / f64::from(tile_size),
        tile_size,
        tiles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tengri_geo::{PointE5, mercator_bounds};

    /// What every provider we've pointed at so far cuts.
    const TILE_SIZE: u32 = 256;

    /// A flight `span_m` across, somewhere in the Alps.
    fn layout_for(span_m: f64) -> Layout {
        let corner = PointE5::new(45_30000, 6_05000);
        let degrees = (span_m / 111_320.0 * 1e5) as i32;
        let bounds = mercator_bounds(&[
            corner,
            PointE5::new(corner.lat + degrees, corner.lon + degrees),
        ])
        .unwrap();
        Layout::new(bounds)
    }

    #[test]
    fn a_long_flight_zooms_out_further_than_a_short_one() {
        let short = plan(&layout_for(5_000.0), TILE_SIZE).unwrap();
        let long = plan(&layout_for(200_000.0), TILE_SIZE).unwrap();

        assert!(short.tiles[0].z > long.tiles[0].z);
        assert_eq!(long.tiles[0].z, 7);
    }

    #[test]
    fn a_tiny_flight_stops_at_the_deepest_zoom() {
        let plan = plan(&layout_for(200.0), TILE_SIZE).unwrap();

        assert_eq!(plan.tiles[0].z, MAX_ZOOM);
    }

    /// Bigger tiles cover the same canvas at a shallower zoom, so the imagery
    /// stays the same size on screen however the service cuts it.
    #[test]
    fn a_retina_service_zooms_out_one_step() {
        let plan_256 = plan(&layout_for(25_000.0), TILE_SIZE).unwrap();
        let plan_512 = plan(&layout_for(25_000.0), 2 * TILE_SIZE).unwrap();

        assert_eq!(plan_512.tiles[0].z, plan_256.tiles[0].z - 1);
        assert!((plan_512.metres_per_px - plan_256.metres_per_px).abs() < 1e-9);
    }

    #[test]
    fn every_plan_stays_within_budget() {
        for span_m in [200.0, 1_000.0, 5_000.0, 25_000.0, 120_000.0, 400_000.0] {
            let plan = plan(&layout_for(span_m), TILE_SIZE).unwrap();

            assert!(
                plan.tiles.len() <= MAX_TILES,
                "{span_m} m: {}",
                plan.tiles.len()
            );
            assert_eq!(plan.tiles.len(), (plan.columns * plan.rows) as usize);
        }
    }

    #[test]
    fn the_mosaic_covers_the_padded_canvas() {
        let layout = layout_for(25_000.0);
        let covered = layout.covered_bounds();

        let plan = plan(&layout, TILE_SIZE).unwrap();

        let tile_m = plan.metres_per_px * f64::from(plan.tile_size);
        let width_m = f64::from(plan.columns) * tile_m;
        let height_m = f64::from(plan.rows) * tile_m;
        assert!(plan.origin.x <= covered.min_x);
        assert!(plan.origin.x + width_m >= covered.max_x);
        assert!(plan.origin.y >= covered.max_y);
        assert!(plan.origin.y - height_m <= covered.min_y);
    }
}
