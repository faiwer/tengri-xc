//! Stitching the tiles into one image, and putting that image on the canvas.

use tengri_geo::{Point, XyzTile, tile_origin_m};
use tiny_skia::{BlendMode, FilterQuality, Pixmap, PixmapPaint, Transform};

use super::tiles::TilePlan;
use crate::flight::image::layout::Layout;

/// Faint enough that the track and its route stay the subject.
const OPACITY: f32 = 0.8;

/// A stitched satellite mosaic, big enough to cover the whole canvas.
pub struct Basemap {
    image: Pixmap,
    /// North-west corner of `image`, in Mercator metres.
    origin: Point,
    metres_per_px: f64,
}

/// `None` if a tile isn't the size `SATELLITE_MAP_TILE_SIZE` promised: the
/// offsets are spaced by the configured size, so a mismatch would stack the
/// tiles on top of each other instead of beside each other.
pub(super) fn stitch(plan: &TilePlan, tiles: &[(XyzTile, Pixmap)]) -> Option<Basemap> {
    let side = plan.tile_size;
    let mut image = Pixmap::new(plan.columns * side, plan.rows * side)?;
    for (tile, source) in tiles {
        if source.width() != side || source.height() != side {
            return None;
        }
        let origin = tile_origin_m(*tile);
        image.draw_pixmap(
            ((origin.x - plan.origin.x) / plan.metres_per_px).round() as i32,
            ((plan.origin.y - origin.y) / plan.metres_per_px).round() as i32,
            source.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    Some(Basemap {
        image,
        origin: plan.origin,
        metres_per_px: plan.metres_per_px,
    })
}

/// Drawn in one scaled blit rather than tile by tile: scaling each tile on its
/// own leaves seams where the filter runs out of neighbours.
pub(in crate::flight::image) fn draw_basemap(
    pixmap: &mut Pixmap,
    layout: &Layout,
    basemap: &Basemap,
) {
    let scale = (layout.scale() * basemap.metres_per_px) as f32;
    let (x, y) = layout.to_canvas(basemap.origin);
    pixmap.draw_pixmap(
        0,
        0,
        basemap.image.as_ref(),
        &PixmapPaint {
            opacity: OPACITY,
            blend_mode: BlendMode::SourceOver,
            quality: FilterQuality::Bilinear,
        },
        Transform::from_row(scale, 0.0, 0.0, scale, x, y),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::image::basemap::tiles::plan;
    use tengri_geo::{PointE5, mercator_bounds};

    const TILE_SIZE: u32 = 256;

    fn solid(side: u32, red: u8, green: u8, blue: u8) -> Pixmap {
        let mut pixmap = Pixmap::new(side, side).unwrap();
        pixmap.fill(tiny_skia::Color::from_rgba8(red, green, blue, 0xff));
        pixmap
    }

    fn alpine_layout() -> Layout {
        let bounds = mercator_bounds(&[
            PointE5::new(45_30000, 6_05000),
            PointE5::new(45_50000, 6_35000),
        ])
        .unwrap();
        Layout::new(bounds)
    }

    #[test]
    fn each_tile_lands_in_its_own_cell() {
        let layout = alpine_layout();
        let plan = plan(&layout, TILE_SIZE).unwrap();
        let tiles: Vec<_> = plan
            .tiles
            .iter()
            .enumerate()
            .map(|(index, &tile)| (tile, solid(TILE_SIZE, index as u8 * 40, 0, 0)))
            .collect();

        let basemap = stitch(&plan, &tiles).unwrap();

        assert_eq!(basemap.image.width(), plan.columns * TILE_SIZE);
        for (index, _) in tiles.iter().enumerate() {
            let column = index as u32 % plan.columns;
            let row = index as u32 / plan.columns;
            let pixel = basemap.image.pixels()
                [(row * TILE_SIZE * basemap.image.width() + column * TILE_SIZE) as usize];
            assert_eq!(pixel.red(), index as u8 * 40, "tile {index}");
        }
    }

    #[test]
    fn a_tile_of_the_wrong_size_loses_the_backdrop() {
        let layout = alpine_layout();
        let plan = plan(&layout, TILE_SIZE).unwrap();
        let tiles: Vec<_> = plan
            .tiles
            .iter()
            .map(|&tile| (tile, solid(2 * TILE_SIZE, 0xff, 0, 0)))
            .collect();

        assert!(stitch(&plan, &tiles).is_none());
    }

    #[test]
    fn the_backdrop_tints_the_canvas_without_covering_it() {
        let layout = alpine_layout();
        let plan = plan(&layout, TILE_SIZE).unwrap();
        let tiles: Vec<_> = plan
            .tiles
            .iter()
            .map(|&tile| (tile, solid(TILE_SIZE, 0xff, 0, 0)))
            .collect();
        let basemap = stitch(&plan, &tiles).unwrap();
        let mut canvas = layout.canvas().unwrap();

        draw_basemap(&mut canvas, &layout, &basemap);

        let middle =
            canvas.pixels()[(canvas.height() / 2 * canvas.width() + canvas.width() / 2) as usize];
        assert_eq!(middle.red(), 0xff, "red stays saturated");
        let white_through = (255.0 * (1.0 - OPACITY)) as u8;
        assert!(
            middle.green().abs_diff(white_through) <= 2,
            "at {OPACITY} opacity white should show through as ~{white_through}, got {}",
            middle.green()
        );
    }
}
