//! Quantisation of vario into 1 m/s buckets.

/// Bucket `-5` covers every sink at or below -5 m/s.
pub const MIN_BUCKET: i8 = -5;
/// Bucket `+5` covers every climb at or above +5 m/s.
pub const MAX_BUCKET: i8 = 5;

/// Quantise vario (m/s) into buckets at 1 m/s resolution, clamped to
/// `[MIN_BUCKET, MAX_BUCKET]`. Bucket `k` is the half-open range
/// `[k, k + 1)` m/s.
pub fn classify_buckets(vario: &[f32]) -> Vec<i8> {
    vario
        .iter()
        .map(|value| {
            (value.floor() as i32).clamp(i32::from(MIN_BUCKET), i32::from(MAX_BUCKET)) as i8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bucket_spans_one_metre_per_second() {
        assert_eq!(classify_buckets(&[0.0, 0.9, 1.0, -0.1]), vec![0, 0, 1, -1]);
    }

    #[test]
    fn extremes_clamp_to_the_ramp_ends() {
        assert_eq!(
            classify_buckets(&[-40.0, 40.0]),
            vec![MIN_BUCKET, MAX_BUCKET]
        );
    }
}
