const REABSORB_INTERIOR_SECONDS = 8;
const MIN_SEGMENT_SECONDS = 15;

export interface VarioSegment {
  /** Quantised vario in whole m/s; range `[-5, +5]`. See `classify.ts`. */
  bucket: number;
  /** Inclusive start index. */
  startIdx: number;
  /** Exclusive end index. */
  endIdx: number;
}

/**
 * Group per-fix bucket values into colour-worthy segments.
 *
 * Three passes, each cheap and order-independent:
 *
 *   1. Coalesce maximal runs of the same bucket.
 *   2. **Reabsorb** runs ≤ REABSORB_INTERIOR_SECONDS s sandwiched between two
 *      runs of the same bucket — small disruptions don't cut a run in two.
 *   3. **Dissolve** any remaining run < MIN_SEGMENT_SECONDS s into its larger
 *      neighbour, on the principle that a short isolated blip belongs to
 *      whichever side dominates around it. Iterates until no short runs remain.
 *
 * Each surviving segment carries the integer bucket; the renderer decides the
 * colour. Times come from `track.t` (Unix epoch seconds), so duration
 * thresholds are in real seconds regardless of sample rate.
 */
export const buildVarioSegments = (
  buckets: Int8Array,
  times: Uint32Array,
  fromIdx: number,
  toIdx: number,
): VarioSegment[] => {
  if (toIdx <= fromIdx) {
    return [];
  }

  let segments = coalesce(buckets, fromIdx, toIdx);
  segments = reabsorbInteriorRuns(segments, times);
  segments = dissolveShortRuns(segments, times);
  return segments;
};

const coalesce = (
  buckets: Int8Array,
  fromIdx: number,
  toIdx: number,
): VarioSegment[] => {
  const segments: VarioSegment[] = [];
  let runStart = fromIdx;
  let runBucket = buckets[fromIdx]!;
  for (let i = fromIdx + 1; i < toIdx; i++) {
    if (buckets[i] !== runBucket) {
      segments.push({ bucket: runBucket, startIdx: runStart, endIdx: i });
      runStart = i;
      runBucket = buckets[i]!;
    }
  }
  segments.push({ bucket: runBucket, startIdx: runStart, endIdx: toIdx });
  return segments;
};

const durationSeconds = (segment: VarioSegment, times: Uint32Array): number => {
  const last = segment.endIdx - 1;
  return times[last]! - times[segment.startIdx]!;
};

const reabsorbInteriorRuns = (
  segments: VarioSegment[],
  times: Uint32Array,
): VarioSegment[] => {
  if (segments.length < 3) {
    return segments;
  }

  const out: VarioSegment[] = [segments[0]!];
  for (let i = 1; i < segments.length - 1; i++) {
    const prev = out[out.length - 1]!;
    const cur = segments[i]!;
    const next = segments[i + 1]!;
    if (
      prev.bucket === next.bucket &&
      cur.bucket !== prev.bucket &&
      durationSeconds(cur, times) <= REABSORB_INTERIOR_SECONDS
    ) {
      out[out.length - 1] = {
        bucket: prev.bucket,
        startIdx: prev.startIdx,
        endIdx: next.endIdx,
      };
      i++;
    } else {
      out.push(cur);
    }
  }
  const last = segments[segments.length - 1]!;
  if (out[out.length - 1]!.endIdx !== last.endIdx) {
    out.push(last);
  }
  return out;
};

/**
 * Iteratively merge each sub-MIN_SEGMENT_SECONDS s run into its larger
 * neighbour and re-coalesce if the merge produced adjacent same-bucket runs.
 * Worst-case O(n²) but n is the number of segments after pass 1, which is small
 * (hundreds, not thousands).
 */
const dissolveShortRuns = (
  segments: VarioSegment[],
  times: Uint32Array,
): VarioSegment[] => {
  const out = segments.slice();

  let madeProgress = true;
  while (madeProgress) {
    madeProgress = false;
    for (let i = 0; i < out.length; i++) {
      if (durationSeconds(out[i]!, times) >= MIN_SEGMENT_SECONDS) {
        continue;
      }

      const neighbour = pickLargerNeighbour(out, i, times);
      if (neighbour === null) {
        break;
      }

      const merged: VarioSegment = {
        bucket: out[neighbour]!.bucket,
        startIdx: Math.min(out[i]!.startIdx, out[neighbour]!.startIdx),
        endIdx: Math.max(out[i]!.endIdx, out[neighbour]!.endIdx),
      };
      const lo = Math.min(i, neighbour);
      const hi = Math.max(i, neighbour);
      out.splice(lo, hi - lo + 1, merged);
      mergeWithSameBucketNeighbours(out, lo);
      madeProgress = true;
      break;
    }
  }

  return out;
};

const pickLargerNeighbour = (
  segments: VarioSegment[],
  i: number,
  times: Uint32Array,
): number | null => {
  const left = i - 1 >= 0 ? i - 1 : null;
  const right = i + 1 < segments.length ? i + 1 : null;
  if (left === null && right === null) {
    return null;
  }

  if (left === null) {
    return right;
  }

  if (right === null) {
    return left;
  }

  return durationSeconds(segments[right]!, times) >
    durationSeconds(segments[left]!, times)
    ? right
    : left;
};

const mergeWithSameBucketNeighbours = (
  segments: VarioSegment[],
  i: number,
): void => {
  if (
    i + 1 < segments.length &&
    segments[i]!.bucket === segments[i + 1]!.bucket
  ) {
    segments[i] = {
      bucket: segments[i]!.bucket,
      startIdx: segments[i]!.startIdx,
      endIdx: segments[i + 1]!.endIdx,
    };
    segments.splice(i + 1, 1);
  }

  if (i - 1 >= 0 && segments[i - 1]!.bucket === segments[i]!.bucket) {
    segments[i - 1] = {
      bucket: segments[i - 1]!.bucket,
      startIdx: segments[i - 1]!.startIdx,
      endIdx: segments[i]!.endIdx,
    };
    segments.splice(i, 1);
  }
};
