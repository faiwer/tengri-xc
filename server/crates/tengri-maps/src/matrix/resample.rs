/// Area-weighted resample of a row-major grid.
///
/// Each output pixel is the weighted average of every source pixel its
/// rectangular footprint touches, weighted by the overlap area. Source edges
/// land on output edges (no half-pixel offset), so resampling adjacent tiles
/// produces aligned borders.
///
/// `sample` lifts each source value into `f64` for the average. Callers cast
/// the resulting `f64` back to whatever target type they store (with their own
/// rounding and clamping policy). When `dst_w == src_w` and `dst_h == src_h`
/// the output is still computed pixel-by-pixel; callers wanting a true no-op
/// should detect that case themselves.
///
/// # Panics
///
/// Panics if `source.len() != src_w * src_h`, or if `src_w == 0` or `src_h ==
/// 0` (all weights are zero, division by zero).
pub fn area_resample<T: Copy>(
    source: &[T],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
    sample: impl Fn(T) -> f64,
) -> Vec<f64> {
    assert_eq!(
        source.len(),
        src_w * src_h,
        "area_resample: source length does not match src_w * src_h"
    );

    let mut output = Vec::with_capacity(dst_w * dst_h);
    for y in 0..dst_h {
        let y_range = source_range(y, dst_h, src_h);
        let y_start = y_range.0.floor() as usize;
        let y_end = y_range.1.ceil() as usize;
        for x in 0..dst_w {
            let x_range = source_range(x, dst_w, src_w);
            let x_start = x_range.0.floor() as usize;
            let x_end = x_range.1.ceil() as usize;
            let mut weighted_sum = 0.0;
            let mut weight_sum = 0.0;

            for src_y in y_start..y_end.min(src_h) {
                let y_weight = overlap(y_range, src_y);
                for src_x in x_start..x_end.min(src_w) {
                    let weight = overlap(x_range, src_x) * y_weight;
                    weighted_sum += sample(source[src_y * src_w + src_x]) * weight;
                    weight_sum += weight;
                }
            }

            output.push(weighted_sum / weight_sum);
        }
    }
    output
}

fn source_range(output_idx: usize, output_len: usize, source_len: usize) -> (f64, f64) {
    let scale = source_len as f64 / output_len as f64;
    let start = output_idx as f64 * scale;
    (start, start + scale)
}

fn overlap(range: (f64, f64), idx: usize) -> f64 {
    let start = range.0.max(idx as f64);
    let end = range.1.min(idx as f64 + 1.0);
    (end - start).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_dimensions_pass_values_through_within_rounding() {
        let source: Vec<f64> = (0..6).map(|v| v as f64).collect();
        let resampled = area_resample(&source, 3, 2, 3, 2, |v| v);

        for (out, &src) in resampled.iter().zip(&source) {
            assert!((out - src).abs() < 1e-9);
        }
    }

    #[test]
    fn halving_width_averages_adjacent_pixels() {
        let source = [1.0, 3.0, 5.0, 7.0];
        let resampled = area_resample(&source, 4, 1, 2, 1, |v| v);

        assert!((resampled[0] - 2.0).abs() < 1e-9);
        assert!((resampled[1] - 6.0).abs() < 1e-9);
    }

    #[test]
    fn output_edges_align_with_source_edges() {
        let source: Vec<f64> = (0..512).map(|v| v as f64).collect();
        let resampled = area_resample(&source, 512, 1, 256, 1, |v| v);

        assert!((resampled[0] - 0.5).abs() < 1e-9);
        assert!((resampled[255] - 510.5).abs() < 1e-9);
    }

    #[test]
    fn sample_closure_is_applied_to_each_source_value() {
        let source: Vec<i16> = vec![-2, 4, -6, 8];
        let resampled = area_resample(&source, 2, 2, 1, 1, |v| f64::from(v.max(0)));

        assert!((resampled[0] - 3.0).abs() < 1e-9);
    }
}
