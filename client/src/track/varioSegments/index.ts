import type { Track } from '../types';
import { classifyBuckets } from './classify';
import { peakVario, type VarioPeaks } from './peakVario';
import { buildVarioSegments, type VarioSegment } from './segments';
import { computeVario } from './vario';

export type { VarioSegment } from './segments';
export type { VarioPeaks } from './peakVario';
export { MIN_BUCKET, MAX_BUCKET } from './classify';

export interface VarioInsights extends VarioPeaks {
  /** Coloured segments covering `[fromIdx, toIdx)`. */
  segments: VarioSegment[];
}

/**
 * End-to-end vario pipeline over the half-open range `[fromIdx, toIdx)`:
 * smoothed vario → quantised buckets → merged colour-worthy segments,
 * plus the strongest climb/sink observed in the same range. Sharing the
 * single `computeVario` call keeps the rendering and the panel stats in
 * lock-step (same window, same smoothing) at no extra cost.
 */
export const computeVarioInsights = (
  track: Track,
  fromIdx: number,
  toIdx: number,
): VarioInsights => {
  const vario = computeVario(track);
  const buckets = classifyBuckets(vario);
  const segments = buildVarioSegments(buckets, track.t, fromIdx, toIdx);
  const { peakClimb, peakSink } = peakVario(vario, fromIdx, toIdx);
  return { segments, peakClimb, peakSink };
};
