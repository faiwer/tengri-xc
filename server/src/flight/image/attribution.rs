//! The imagery provider's credit: one line of small dark text in a translucent
//! white box, flush to the bottom-right corner.

use std::sync::OnceLock;

use ab_glyph::{Font, FontRef, ScaleFont, point};
use tiny_skia::{Mask, Pixmap, Rect, Transform};

use super::paint::{Rgb, WHITE, brush, solid_brush};

/// Bundled rather than read at runtime: the runtime image carries no fonts,
/// and a missing file would show up only as a silently absent credit.
const FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/fonts/Roboto-Regular.subset.ttf"
));
const FONT_SIZE_PX: f32 = 14.0;
const PADDING_X: f32 = 10.0;
const PADDING_Y: f32 = 3.0;
/// Short of black, so the line reads as a caption and not as drawn data.
const INK: Rgb = (0x22, 0x22, 0x22);
const BOX_ALPHA: u8 = 0x80;

pub(super) fn draw_attribution(pixmap: &mut Pixmap, text: &str) {
    let Some(font) = font() else {
        return;
    };
    let font = font.as_scaled(FONT_SIZE_PX);

    let text_width = text
        .chars()
        .map(|character| font.h_advance(font.glyph_id(character)))
        .sum::<f32>();
    if text_width <= 0.0 {
        return;
    }

    let width = pixmap.width() as f32;
    let height = pixmap.height() as f32;
    // An overlong credit clips at the right edge rather than starting off-canvas.
    let left = (width - (text_width + 2.0 * PADDING_X)).max(0.0);
    let top = (height - (font.ascent() - font.descent() + 2.0 * PADDING_Y)).max(0.0);
    let Some(label) = Rect::from_ltrb(left, top, width, height) else {
        return;
    };

    pixmap.fill_rect(label, &brush(WHITE, BOX_ALPHA), Transform::identity(), None);

    let Some(mask) = glyph_mask(
        &font,
        text,
        (left + PADDING_X, top + PADDING_Y + font.ascent()),
        pixmap.width(),
        pixmap.height(),
    ) else {
        return;
    };
    // Compositing through a mask beats blending each glyph pixel by hand.
    pixmap.fill_rect(label, &solid_brush(INK), Transform::identity(), Some(&mask));
}

/// Coverage for every glyph of `text`, laid out from `origin` on the baseline.
/// No shaping and no kerning — Roboto keeps its kerning in GPOS, which
/// `ab_glyph` doesn't read, and a credit line is plain Latin text anyway.
fn glyph_mask<F: Font, S: ScaleFont<F>>(
    font: &S,
    text: &str,
    origin: (f32, f32),
    width: u32,
    height: u32,
) -> Option<Mask> {
    let mut mask = Mask::new(width, height)?;
    let data = mask.data_mut();

    let (mut pen, baseline) = origin;
    for character in text.chars() {
        let id = font.glyph_id(character);
        let glyph = id.with_scale_and_position(font.scale(), point(pen, baseline));
        pen += font.h_advance(id);

        let Some(outline) = font.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outline.px_bounds();
        outline.draw(|x, y, coverage| {
            let x = bounds.min.x as i32 + x as i32;
            let y = bounds.min.y as i32 + y as i32;
            if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                return;
            }
            let index = y as usize * width as usize + x as usize;
            data[index] = data[index].max((coverage * 255.0) as u8);
        });
    }

    Some(mask)
}

/// Parsed once; re-parsing per render buys nothing.
fn font() -> Option<&'static FontRef<'static>> {
    static FONT_REF: OnceLock<Option<FontRef<'static>>> = OnceLock::new();
    FONT_REF
        .get_or_init(|| FontRef::try_from_slice(FONT).ok())
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Color;

    fn canvas(width: u32, height: u32, color: Color) -> Pixmap {
        let mut pixmap = Pixmap::new(width, height).unwrap();
        pixmap.fill(color);
        pixmap
    }

    fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> tiny_skia::PremultipliedColorU8 {
        pixmap.pixels()[(y * pixmap.width() + x) as usize]
    }

    #[test]
    fn the_credit_lands_in_the_bottom_right_corner() {
        let mut pixmap = canvas(500, 300, Color::BLACK);

        draw_attribution(&mut pixmap, "Source: Esri, Maxar");

        assert!(
            pixel(&pixmap, 490, 290).red() > 0x40,
            "the box should lighten the corner it sits in"
        );
        assert_eq!(pixel(&pixmap, 10, 10).red(), 0, "the top-left is untouched");
        assert_eq!(
            pixel(&pixmap, 10, 290).red(),
            0,
            "a short credit leaves the bottom-left alone"
        );
    }

    #[test]
    fn the_glyphs_are_darker_than_their_box() {
        let mut pixmap = canvas(500, 300, Color::WHITE);
        let before = pixmap.data().to_vec();

        draw_attribution(&mut pixmap, "Source: Esri, Maxar");

        // The box is white on white; only the ink can have changed anything.
        assert_ne!(pixmap.data(), before.as_slice());
        let darkest = pixmap.pixels().iter().map(|pixel| pixel.red()).min();
        assert!(darkest.unwrap() < 0x80, "{darkest:?}");
    }

    #[test]
    fn an_empty_credit_draws_nothing() {
        let mut pixmap = canvas(500, 300, Color::BLACK);
        let before = pixmap.data().to_vec();

        draw_attribution(&mut pixmap, "");

        assert_eq!(pixmap.data(), before.as_slice());
    }

    #[test]
    fn an_overlong_credit_keeps_its_box_on_the_canvas() {
        let mut pixmap = canvas(200, 200, Color::BLACK);

        draw_attribution(
            &mut pixmap,
            &"Esri, Maxar, Earthstar Geographics, ".repeat(4),
        );

        assert!(
            pixel(&pixmap, 0, 190).red() > 0x40,
            "the box should reach the left edge instead of running off the right"
        );
        assert_eq!(pixel(&pixmap, 0, 10).red(), 0, "the top-left is untouched");
    }
}
