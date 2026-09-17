use tiny_skia::Paint;

/// 8-bit RGB, spelled like the client's colour constants.
pub(super) type Rgb = (u8, u8, u8);

pub(super) const WHITE: Rgb = (0xff, 0xff, 0xff);
/// The client's `SCORED_COLOR`.
pub(super) const SCORED: Rgb = (0x65, 0xc8, 0x32);
/// The client's `UNSCORED_COLOR`, for what was flown but doesn't count.
pub(super) const UNSCORED: Rgb = (0xd8, 0x9a, 0x12);

/// The brush `fill_path` / `stroke_path` take: one flat opaque colour,
/// antialiased. Everything we draw is a flat colour.
pub(super) fn solid_brush<'a>(color: Rgb) -> Paint<'a> {
    brush(color, 0xff)
}

/// [`solid_brush`] with an alpha, for what should let the imagery through.
pub(super) fn brush<'a>(color: Rgb, alpha: u8) -> Paint<'a> {
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color_rgba8(color.0, color.1, color.2, alpha);
    paint
}
