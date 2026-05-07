import type { Track } from '../types';
import { classifyBuckets } from './classify';
import { buildVarioSegments, type VarioSegment } from './segments';
import { computeVario } from './vario';

export type { VarioSegment } from './segments';
export { MIN_BUCKET, MAX_BUCKET } from './classify';

/**
 * End-to-end vario-segment pipeline: smoothed vario → quantised buckets →
 * merged colour-worthy segments. The `[fromIdx, toIdx)` range bounds the
 * segmentation to the flight portion of the track (anything outside the
 * detected takeoff/landing window stays gray and isn't classified).
 */
export const computeVarioSegments = (
  track: Track,
  fromIdx: number,
  toIdx: number,
): VarioSegment[] => {
  const vario = computeVario(track);
  const buckets = classifyBuckets(vario);
  return buildVarioSegments(buckets, track.t, fromIdx, toIdx);
};
