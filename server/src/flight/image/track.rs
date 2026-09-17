//! The flown track as a polyline.

use tengri_geo::{Point, rdp_indexes_with_chord_cap};
use tiny_skia::{LineCap, LineJoin, PathBuilder, Pixmap, Stroke, Transform};

use super::{
    layout::Layout,
    paint::{Rgb, solid_brush},
};

/// A fix further than this from the chord between its neighbours is kept.
const RDP_TOLERANCE_M: f64 = 200.0;
const WIDTH_PX: f32 = 2.0;
/// The client's `COLOR_MISSING_ALTITUDE`.
const COLOR: Rgb = (0x3b, 0x82, 0xf6);

pub(super) fn simplify(points: &[Point]) -> Vec<Point> {
    rdp_indexes_with_chord_cap(points, RDP_TOLERANCE_M, None)
        .into_iter()
        .map(|idx| points[idx])
        .collect()
}

pub(super) fn draw_track(pixmap: &mut Pixmap, layout: &Layout, points: &[Point]) {
    let mut path = PathBuilder::new();
    for (idx, point) in points.iter().enumerate() {
        let (x, y) = layout.to_canvas(*point);
        if idx == 0 {
            path.move_to(x, y);
        } else {
            path.line_to(x, y);
        }
    }
    // A single-fix track has no segment to stroke.
    let Some(path) = path.finish() else {
        return;
    };

    let stroke = Stroke {
        width: WIDTH_PX,
        // Thermal turns come out spiky on a mitre join.
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    pixmap.stroke_path(
        &path,
        &solid_brush(COLOR),
        &stroke,
        Transform::identity(),
        None,
    );
}
