use tiny_skia::Paint;

/// 8-bit RGB, spelled like the client's colour constants.
pub(super) type Rgb = (u8, u8, u8);

pub(super) const WHITE: Rgb = (0xff, 0xff, 0xff);

/// The brush `fill_path` / `stroke_path` take: one flat opaque colour,
/// antialiased. Everything we draw is a flat colour.
pub(super) fn solid_brush<'a>(color: Rgb) -> Paint<'a> {
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color_rgba8(color.0, color.1, color.2, 0xff);
    paint
}
