/**
 * Centred moving average over a time-aware window: each output sample
 * is the mean of all input samples whose timestamp falls within
 * `±halfSeconds` of the centre fix.
 *
 * Time-aware (vs. fixed `±N samples`) is the right choice for IGC data:
 * sampling rate varies across instruments (1 Hz on most GPS loggers, 4
 * Hz on some Flymasters, irregular on GpsDump exports) and a fixed-N
 * window would smooth those tracks at very different physical scales.
 *
 * The window edges are clamped to the array bounds, so the first/last
 * `halfSeconds` of samples use an asymmetric, narrower window. After
 * downstream chart-bucketing into ~1500 samples those edges each cover
 * several seconds anyway, so the asymmetry stays invisible.
 *
 * Two-pointer sweep, single allocation, O(n).
 */
export const movingAverage = (
  times: Uint32Array | number[],
  values: Float32Array,
  halfSeconds: number,
): Float32Array => {
  const n = values.length;
  const out = new Float32Array(n);
  if (n === 0) {
    return out;
  }

  // Track the running window sum incrementally — adding the new right
  // edge and dropping the new left edge each step — so each fix costs
  // O(1) instead of O(window).
  let lo = 0;
  let hi = -1;
  let sum = 0;
  for (let i = 0; i < n; i++) {
    const tCenter = times[i]!;
    const tLo = tCenter - halfSeconds;
    const tHi = tCenter + halfSeconds;

    while (hi + 1 < n && times[hi + 1]! <= tHi) {
      hi++;
      sum += values[hi]!;
    }
    while (lo < n && times[lo]! < tLo) {
      sum -= values[lo]!;
      lo++;
    }

    const span = hi - lo + 1;
    // span ≥ 1 always: the centre fix itself is in-window because
    // tLo ≤ times[i] ≤ tHi, so hi ≥ i ≥ lo.
    out[i] = sum / span;
  }

  return out;
};
