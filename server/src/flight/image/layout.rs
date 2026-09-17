//! The Mercator metre plane mapped onto canvas pixels.

use anyhow::anyhow;
use tengri_geo::{MercatorRect, Point};
use tiny_skia::{Color, Pixmap};

/// Longer side of the drawing area, before padding.
const MAX_SIDE_PX: f64 = 400.0;
/// Added to every side, so the canvas tops out at 500 px.
const PADDING_PX: f64 = 50.0;

pub(super) struct Layout {
    width: u32,
    height: u32,
    /// Pixels per metre.
    scale: f64,
    min_x: f64,
    /// Canvas y grows downward, so the northernmost metre is the origin.
    max_y: f64,
}

impl Layout {
    /// Sized to `bounds`; anything drawn outside it lands in the padding, or
    /// off the canvas entirely.
    pub(super) fn new(bounds: MercatorRect) -> Self {
        let span_x = bounds.width();
        let span_y = bounds.height();
        let longest = span_x.max(span_y);
        // A single fix (or a pilot who never moved) has nothing to scale to.
        let scale = if longest > 0.0 {
            MAX_SIDE_PX / longest
        } else {
            1.0
        };

        Self {
            width: canvas_side(span_x * scale),
            height: canvas_side(span_y * scale),
            scale,
            min_x: bounds.min_x,
            max_y: bounds.max_y,
        }
    }

    /// Convert a Mercator metre point to canvas pixels.
    pub(super) fn to_canvas(&self, point: Point) -> (f32, f32) {
        (
            (PADDING_PX + (point.x - self.min_x) * self.scale) as f32,
            (PADDING_PX + (self.max_y - point.y) * self.scale) as f32,
        )
    }

    /// Pixels per Mercator metre.
    pub(super) fn scale(&self) -> f64 {
        self.scale
    }

    /// What the whole canvas covers, padding included — the area a backdrop
    /// has to fill, which is wider than the track's own box.
    pub(super) fn covered_bounds(&self) -> MercatorRect {
        let padding_m = PADDING_PX / self.scale;
        MercatorRect::new(
            self.min_x - padding_m,
            self.max_y + padding_m - f64::from(self.height) / self.scale,
            self.min_x - padding_m + f64::from(self.width) / self.scale,
            self.max_y + padding_m,
        )
    }

    pub(super) fn canvas(&self) -> anyhow::Result<Pixmap> {
        let mut pixmap = Pixmap::new(self.width, self.height)
            .ok_or_else(|| anyhow!("{}x{} canvas rejected", self.width, self.height))?;
        pixmap.fill(Color::WHITE);
        Ok(pixmap)
    }
}

fn canvas_side(drawn_px: f64) -> u32 {
    (drawn_px.ceil() + 2.0 * PADDING_PX) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> MercatorRect {
        MercatorRect::new(min_x, min_y, max_x, max_y)
    }

    #[test]
    fn the_longer_span_sets_the_scale() {
        let wide = Layout::new(bounds(0.0, 0.0, 10_000.0, 2_500.0));

        assert_eq!(wide.width, 500);
        assert_eq!(wide.height, 200);
    }

    #[test]
    fn a_tall_track_caps_its_height() {
        let tall = Layout::new(bounds(0.0, 0.0, 2_500.0, 10_000.0));

        assert_eq!(tall.width, 200);
        assert_eq!(tall.height, 500);
    }

    #[test]
    fn a_stationary_track_is_all_padding() {
        let still = Layout::new(bounds(12.0, -4.0, 12.0, -4.0));

        assert_eq!(still.width, 100);
        assert_eq!(still.height, 100);
        assert_eq!(still.to_canvas(Point::new(12.0, -4.0)), (50.0, 50.0));
    }

    #[test]
    fn north_is_up() {
        let layout = Layout::new(bounds(0.0, 0.0, 0.0, 10_000.0));

        let (_, south) = layout.to_canvas(Point::new(0.0, 0.0));
        let (_, north) = layout.to_canvas(Point::new(0.0, 10_000.0));
        assert!(north < south);
    }

    #[test]
    fn the_covered_bounds_are_the_canvas_corners() {
        let layout = Layout::new(bounds(0.0, 0.0, 10_000.0, 2_500.0));

        let covered = layout.covered_bounds();

        assert_eq!(
            layout.to_canvas(Point::new(covered.min_x, covered.max_y)),
            (0.0, 0.0)
        );
        let (right, bottom) = layout.to_canvas(Point::new(covered.max_x, covered.min_y));
        assert!((right - layout.width as f32).abs() < 0.01, "{right}");
        assert!((bottom - layout.height as f32).abs() < 0.01, "{bottom}");
    }
}
