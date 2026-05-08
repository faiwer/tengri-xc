import { useMemo } from 'react';
import type { AlignedData } from 'uplot';
import type { Track } from '../../track';
import type { TrackWindow } from '../../track/toPaths';
import { computeVario } from '../../track/varioSegments/vario';
import { bucketMean } from '../../utils/bucketMean';

/**
 * Target chart resolution after mean-bucketing. Same budget as the
 * speed chart — roughly 1.5× the pixel width of the chart container at
 * typical viewport sizes. Below "one segment per pixel" the per-fix
 * thermal wobble (a long flight has thousands of 1 Hz vario samples)
 * collapses into a clean trend instead of a vertical fog inside thermal
 * blocks.
 */
const VARIO_CHART_TARGET_POINTS = 1000;

export interface VarioSeries {
  /**
   * uPlot-shaped series data: `[xs, vario]`. Slot 0 is bucket-centroid
   * epoch seconds (hence `Float64Array`), slot 1 is the smoothed
   * vertical velocity in m/s (positive = climb), bucket-averaged.
   */
  data: AlignedData;
}

/**
 * Build the uPlot data arrays for {@link VarioChart}, sliced to the
 * flight window and mean-bucketed to at most {@link VARIO_CHART_TARGET_POINTS}.
 *
 * `computeVario` runs over the full track because its centred ±5 s window
 * needs neighbours that may live just outside `[takeoffIdx, landedIdx + 1)`;
 * we then slice the result and bucket. No smoothing pad is needed
 * around the slice — the ±5 s vario window is internal to `computeVario`
 * and clamps gracefully at the boundaries (one-sided window at the edges,
 * still a valid local slope).
 */
export const useVarioSeries = (
  track: Track,
  window: TrackWindow,
): VarioSeries => {
  return useMemo((): VarioSeries => {
    const fromIdx = window.takeoffIdx;
    const toIdx = window.landedIdx + 1;
    const xs = track.t.slice(fromIdx, toIdx);
    const vario = computeVario(track).slice(fromIdx, toIdx);
    const bucketed = bucketMean(xs, vario, VARIO_CHART_TARGET_POINTS);
    return { data: [bucketed.xs, bucketed.ys] };
  }, [track, window]);
};
