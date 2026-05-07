/**
 * Quantise per-fix vario into integer buckets at 1 m/s resolution.
 *
 * Bucket value `k` represents the half-open range `[k, k+1)` m/s. Vario
 * values outside the displayable range are clamped — `+5` covers any climb
 * `≥ +5 m/s`, `-5` covers any sink `≤ -5 m/s`. The result spans 11 distinct
 * buckets (`-5..+5` inclusive).
 */
export const MIN_BUCKET = -5;
export const MAX_BUCKET = 5;

export const classifyBuckets = (vario: Float32Array): Int8Array => {
  const buckets = new Int8Array(vario.length);
  for (let i = 0; i < vario.length; i++) {
    let bucket = Math.floor(vario[i]!);
    if (bucket < MIN_BUCKET) {
      bucket = MIN_BUCKET;
    } else if (bucket > MAX_BUCKET) {
      bucket = MAX_BUCKET;
    }
    buckets[i] = bucket;
  }
  return buckets;
};
