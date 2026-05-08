import { useMemo } from 'react';
import type { AlignedData } from 'uplot';
import type { Track } from '../../track';
import { computeGroundSpeed } from '../../track/groundSpeed';
import { computePathSpeed } from '../../track/pathSpeed';
import type { TrackWindow } from '../../track/toPaths';
import { bucketMean } from '../../utils/bucketMean';
import { movingAverage } from '../../utils/movingAverage';

/**
 * Target chart resolution after mean-bucketing. Roughly 1.5× the
 * pixel width of the chart container at typical viewport sizes;
 * dropping per-fix density below "one segment per pixel" is what
 * collapses the dense vertical fog inside thermal blocks into a
 * single trend line.
 */
const SPEED_CHART_TARGET_POINTS = 1500;

/**
 * Half-width of the smoothing window applied to the path-speed and TAS
 * series before bucketing. Same ±30 s window the ground-speed pass
 * already uses internally, so all three lines reach the chart with
 * matching smoothing budgets — without it, mean-bucketing alone leaves
 * each ~5 s bucket carrying ¼-turn of thermal-circle wobble and the
 * raw-per-leg lines read as noise next to the smooth ground line.
 */
const AIRSPEED_SMOOTHING_HALF_SECONDS = 30;

export interface SpeedSeries {
  /**
   * uPlot-shaped series data. Slot 0 is epoch seconds (mean-bucketed
   * centroids, hence `Float64Array`); slot 1 is windowed ground speed
   * in km/h ("GPS"); slot 2 is path speed in km/h ("Path"); slot 3,
   * when present, is the recorded true-airspeed channel in km/h
   * ("TAS"). All three y-series are bucketed against the same x slice,
   * so they line up index-for-index.
   */
  data: AlignedData;
}

/**
 * Build the uPlot data arrays for {@link SpeedChart}, sliced to the
 * flight window, ±30 s-smoothed, and mean-bucketed to at most
 * {@link SPEED_CHART_TARGET_POINTS} samples.
 *
 * Three physical quantities, three sources, three lines:
 * - **GPS** — windowed ground speed via {@link computeGroundSpeed}
 *   (displacement across a centred ±30 s window). Already smooth.
 * - **Path** — per-leg path speed via {@link computePathSpeed}, then
 *   ±30 s {@link movingAverage}-smoothed. Reads ~airspeed in turns,
 *   ~ground speed on glides.
 * - **TAS** — straight from `track.tas`, then ±30 s smoothed for
 *   parity with the path line. Only present when the source IGC
 *   carried a TAS column.
 *
 * The Path line is *always* present so the user can see how well GPS-
 * derived airspeed agrees with the recorded TAS instrument when both
 * are available — the divergence of Path vs TAS in glides and their
 * agreement in thermals is itself a useful sanity check on the
 * instrument's calibration.
 */
export const useSpeedSeries = (
  track: Track,
  window: TrackWindow,
): SpeedSeries => {
  return useMemo((): SpeedSeries => {
    const fromIdx = window.takeoffIdx;
    const toIdx = window.landedIdx + 1;

    // `Float32Array.prototype.slice` produces a fresh, contiguous copy,
    // which `bucketMean` needs (it does no slicing of its own).
    // `subarray` would share the underlying buffer with the full-track
    // result and force the consumer to reason about offsets.
    const gs = computeGroundSpeed(track).slice(fromIdx, toIdx);
    const path = computePathSpeed(track).slice(fromIdx, toIdx);
    const xs = track.t.slice(fromIdx, toIdx);

    const gsBucketed = bucketMean(xs, gs, SPEED_CHART_TARGET_POINTS);
    const pathSmoothed = movingAverage(
      xs,
      path,
      AIRSPEED_SMOOTHING_HALF_SECONDS,
    );
    const pathBucketed = bucketMean(
      xs,
      pathSmoothed,
      SPEED_CHART_TARGET_POINTS,
    );

    if (!track.tas) {
      return { data: [gsBucketed.xs, gsBucketed.ys, pathBucketed.ys] };
    }

    const tas = sliceTas(track, fromIdx, toIdx);
    const tasSmoothed = movingAverage(xs, tas, AIRSPEED_SMOOTHING_HALF_SECONDS);
    const tasBucketed = bucketMean(xs, tasSmoothed, SPEED_CHART_TARGET_POINTS);

    return {
      data: [gsBucketed.xs, gsBucketed.ys, pathBucketed.ys, tasBucketed.ys],
    };
  }, [track, window]);
};

const sliceTas = (
  track: Track,
  fromIdx: number,
  toIdx: number,
): Float32Array => {
  // uPlot wants homogeneous numeric arrays; convert the integer TAS
  // channel to f32 to match the GPS / Path series.
  const length = toIdx - fromIdx;
  const out = new Float32Array(length);
  for (let i = 0; i < length; i++) {
    out[i] = track.tas![fromIdx + i]!;
  }

  return out;
};
