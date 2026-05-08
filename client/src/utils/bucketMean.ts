/**
 * Mean-bucket a parallel pair of arrays into at most `targetBuckets`
 * samples, returning the per-bucket centroid `(mean(x), mean(y))`.
 *
 * Each bucket spans `[lo, hi)` index ranges chosen by integer
 * proportional split (`lo = floor(b·n / B)`); buckets are contiguous,
 * non-overlapping, and exhaustive. Both arrays must have the same
 * length and be sorted by `xs` (they always are when xs is the
 * track's time column).
 *
 * Trade-off vs. min/max envelope decimation: mean-bucketing erases
 * intra-bucket contrast (peak/trough oscillations within one bucket
 * collapse to a single average). For chart consumers that want the
 * "trend" through high-frequency wobble — e.g. ground speed during
 * thermalling, where per-circle peaks and troughs are noise — that
 * erasure is a feature, not a flaw.
 *
 * If the input is already shorter than the target, returns a `Float64`
 * copy of `xs` and the original `ys` untouched (the chart-side cost
 * of an extra copy is negligible at small sizes).
 */
export const bucketMean = (
  xs: Uint32Array,
  ys: Float32Array,
  targetBuckets: number,
): { xs: Float64Array; ys: Float32Array } => {
  const n = xs.length;
  if (n <= targetBuckets || targetBuckets <= 0) {
    return { xs: Float64Array.from(xs), ys };
  }

  const buckets = targetBuckets;
  const outXs = new Float64Array(buckets);
  const outYs = new Float32Array(buckets);

  for (let b = 0; b < buckets; b++) {
    // Integer proportional split keeps the buckets exactly contiguous
    // and exhausts the source: lo of bucket b+1 equals hi of bucket b.
    const lo = Math.floor((b * n) / buckets);
    const hi = Math.floor(((b + 1) * n) / buckets);
    // f64 accumulator for x: epoch-second values in the 1.7e9 range
    // multiplied by ~7 samples comfortably exceed f32's ~24-bit
    // integer-precision floor (~1.7e7).
    let sumX = 0;
    let sumY = 0;
    for (let i = lo; i < hi; i++) {
      sumX += xs[i]!;
      sumY += ys[i]!;
    }
    const len = hi - lo;
    outXs[b] = sumX / len;
    outYs[b] = sumY / len;
  }

  return { xs: outXs, ys: outYs };
};
