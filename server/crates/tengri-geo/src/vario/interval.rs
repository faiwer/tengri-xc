//! Whether a track is sampled densely enough for vario to mean anything.

use super::slope::VARIO_WINDOW_HALF_SECONDS;

/// Coarsest average fix spacing that still yields a usable vario series.
/// [`vario_mps`](super::vario_mps) only admits a neighbour within half a
/// window, so beyond this every fix is alone in its window and the whole
/// series reads as a flat zero — a cliff, not a gradual decay.
pub const MAX_VARIO_FIX_INTERVAL_SECONDS: f64 = VARIO_WINDOW_HALF_SECONDS as f64;

/// Mean seconds between fixes over `times[from_idx..to_idx]`; 0 when the range
/// holds fewer than two fixes.
pub fn average_fix_interval(times: &[u32], from_idx: usize, to_idx: usize) -> f64 {
    let to_idx = to_idx.min(times.len());
    if to_idx <= from_idx + 1 {
        return 0.0;
    }

    let last = to_idx - 1;
    let elapsed = f64::from(times[last].saturating_sub(times[from_idx]));
    elapsed / (last - from_idx) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_hertz_fixes_average_one_second() {
        let times: Vec<u32> = (0..60).collect();

        assert_eq!(average_fix_interval(&times, 0, 60), 1.0);
    }

    #[test]
    fn a_sparse_logger_exceeds_the_vario_limit() {
        let times: Vec<u32> = (0..10).map(|idx| idx * 10).collect();

        assert!(average_fix_interval(&times, 0, 10) > MAX_VARIO_FIX_INTERVAL_SECONDS);
    }

    #[test]
    fn a_single_fix_has_no_interval() {
        assert_eq!(average_fix_interval(&[42], 0, 1), 0.0);
    }
}
