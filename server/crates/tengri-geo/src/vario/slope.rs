//! Per-fix vertical velocity from an altitude column.

/// Half-width of the centred smoothing window.
pub const VARIO_WINDOW_HALF_SECONDS: u32 = 5;

/// Per-fix vertical velocity in m/s, from altitudes in decimetres sampled at
/// `times` (Unix epoch seconds, non-decreasing).
///
/// The centred ±[`VARIO_WINDOW_HALF_SECONDS`] window absorbs second-to-second
/// jitter so callers can classify with simple thresholds. Near the ends it is
/// one-sided: still a valid local slope, just over fewer samples. Feed it
/// barometric altitude when the track has it — it is far smoother than GPS
/// (~±0.1 m vs ±2 m).
pub fn vario_mps(times: &[u32], alt_dm: &[i32]) -> Vec<f32> {
    let count = times.len().min(alt_dm.len());
    let mut vario = vec![0.0; count];
    if count == 0 {
        return vario;
    }

    let mut left = 0;
    let mut right = 0;
    for (idx, &time) in times.iter().enumerate().take(count) {
        let from = time.saturating_sub(VARIO_WINDOW_HALF_SECONDS);
        let to = time + VARIO_WINDOW_HALF_SECONDS;

        while left < count - 1 && times[left] < from {
            left += 1;
        }
        while right < count - 1 && times[right + 1] <= to {
            right += 1;
        }

        let seconds = times[right].saturating_sub(times[left]);
        if seconds == 0 {
            continue;
        }
        let climbed_dm = f64::from(alt_dm[right] - alt_dm[left]);
        vario[idx] = (climbed_dm / 10.0 / f64::from(seconds)) as f32;
    }

    vario
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1 Hz fixes climbing at `rate` m/s from 1000 m.
    fn ramp(count: usize, rate: f64) -> (Vec<u32>, Vec<i32>) {
        let times = (0..count as u32).collect();
        let alt = (0..count)
            .map(|idx| ((1000.0 + idx as f64 * rate) * 10.0).round() as i32)
            .collect();
        (times, alt)
    }

    #[test]
    fn a_constant_altitude_reads_as_zero() {
        let (times, alt) = ramp(13, 0.0);

        for value in vario_mps(&times, &alt) {
            assert_eq!(value, 0.0);
        }
    }

    #[test]
    fn a_steady_climb_reads_as_its_slope() {
        let (times, alt) = ramp(21, 2.0);

        let vario = vario_mps(&times, &alt);

        for value in &vario[5..16] {
            assert!((value - 2.0).abs() < 1e-4, "{value}");
        }
    }

    #[test]
    fn a_steady_sink_reads_negative() {
        let (times, alt) = ramp(21, -1.5);

        let vario = vario_mps(&times, &alt);

        for value in &vario[5..16] {
            assert!((value + 1.5).abs() < 1e-4, "{value}");
        }
    }

    #[test]
    fn the_ends_use_a_one_sided_window() {
        let (times, alt) = ramp(21, 2.0);

        let vario = vario_mps(&times, &alt);

        assert!((vario[0] - 2.0).abs() < 1e-4);
        assert!((vario[vario.len() - 1] - 2.0).abs() < 1e-4);
    }

    #[test]
    fn a_track_shorter_than_the_window_still_reports_a_slope() {
        let (times, alt) = ramp(3, 3.0);

        let vario = vario_mps(&times, &alt);

        assert_eq!(vario.len(), 3);
        for value in vario {
            assert!((value - 3.0).abs() < 1e-4, "{value}");
        }
    }

    #[test]
    fn a_single_fix_has_no_time_delta() {
        assert_eq!(vario_mps(&[0], &[10_000]), vec![0.0]);
    }
}
