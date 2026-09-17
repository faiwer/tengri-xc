//! The projected metre plane mapped onto canvas pixels.

use anyhow::anyhow;
use tengri_geo::Point;
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
    /// Sized to the bounding box of `points`; anything drawn outside it lands
    /// in the padding, or off the canvas entirely.
    pub(super) fn new(points: &[Point]) -> Self {
        let (mut min_x, mut max_x) = (f64::MAX, f64::MIN);
        let (mut min_y, mut max_y) = (f64::MAX, f64::MIN);
        for point in points {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }

        let span_x = max_x - min_x;
        let span_y = max_y - min_y;
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
            min_x,
            max_y,
        }
    }

    /// Convert a projected metre plane point to canvas pixels.
    pub(super) fn to_canvas(&self, point: Point) -> (f32, f32) {
        (
            (PADDING_PX + (point.x - self.min_x) * self.scale) as f32,
            (PADDING_PX + (self.max_y - point.y) * self.scale) as f32,
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

    fn point(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn the_longer_span_sets_the_scale() {
        let wide = Layout::new(&[point(0.0, 0.0), point(10_000.0, 2_500.0)]);

        assert_eq!(wide.width, 500);
        assert_eq!(wide.height, 200);
    }

    #[test]
    fn a_tall_track_caps_its_height() {
        let tall = Layout::new(&[point(0.0, 0.0), point(2_500.0, 10_000.0)]);

        assert_eq!(tall.width, 200);
        assert_eq!(tall.height, 500);
    }

    #[test]
    fn a_stationary_track_is_all_padding() {
        let still = Layout::new(&[point(12.0, -4.0), point(12.0, -4.0)]);

        assert_eq!(still.width, 100);
        assert_eq!(still.height, 100);
        assert_eq!(still.to_canvas(point(12.0, -4.0)), (50.0, 50.0));
    }

    #[test]
    fn north_is_up() {
        let layout = Layout::new(&[point(0.0, 0.0), point(0.0, 10_000.0)]);

        let (_, south) = layout.to_canvas(point(0.0, 0.0));
        let (_, north) = layout.to_canvas(point(0.0, 10_000.0));
        assert!(north < south);
    }
}
