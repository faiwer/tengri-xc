//! The flown track as a polyline, one stroke per colour run.

use tengri_geo::Point;
use tiny_skia::{LineCap, LineJoin, PathBuilder, Pixmap, Stroke, Transform};

use super::{layout::Layout, paint::solid_brush, vario::ColorRun};

const WIDTH_PX: f32 = 2.0;

/// Every fix is a vertex. Simplifying first would hand a thermal's whole
/// spiral to a single chord, and that chord would then have to answer for the
/// several vario runs it crosses.
pub(super) fn draw_track(
    pixmap: &mut Pixmap,
    layout: &Layout,
    points: &[Point],
    runs: &[ColorRun],
) {
    let stroke = Stroke {
        width: WIDTH_PX,
        // Thermal turns come out spiky on a mitre join.
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };

    for run in runs {
        let mut path = PathBuilder::new();
        for (idx, point) in points[run.range.clone()].iter().enumerate() {
            let (x, y) = layout.to_canvas(*point);
            if idx == 0 {
                path.move_to(x, y);
            } else {
                path.line_to(x, y);
            }
        }
        // A single-fix run has no segment to stroke.
        let Some(path) = path.finish() else {
            continue;
        };

        pixmap.stroke_path(
            &path,
            &solid_brush(run.color),
            &stroke,
            Transform::identity(),
            None,
        );
    }
}
