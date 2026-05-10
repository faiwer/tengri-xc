import { useMemo } from 'react';
import type { AlignedData } from 'uplot';
import type { ResolvedPreferences } from '../../core/preferences';
import type { Track } from '../../track';
import { computeGroundSpeed } from '../../track/groundSpeed';
import { computePathSpeed } from '../../track/pathSpeed';
import type { TrackWindow } from '../../track/toPaths';
import { bucketMean } from '../../utils/bucketMean';
import { KMH_TO_MPH } from '../../utils/formatUnits';
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

/**
 * Index pad applied around the flight window before running the speed
 * computations and the ±30 s {@link movingAverage}, then trimmed off
 * before bucketing. Sized to cover both smoothing windows at typical
 * IGC sampling rates (1 Hz on most loggers, 2 Hz on Flymasters; 60
 * indices is a comfortable margin at either rate).
 *
 * Without the pad the takeoff/landing samples would land at the very
 * edge of the compute arrays and have their windows clamped to
 * half-width, lifting the boundary samples slightly. With the pad they
 * see the same neighbour fixes the interior samples do; the recorder's
 * pre-takeoff / post-landing tail (which can be hours of driving home
 * on some tracks) still doesn't enter the result because the pad caps
 * how far we look.
 */
const FLIGHT_SLICE_PAD_FIXES = 60;

export interface SpeedSeries {
  /**
   * uPlot-shaped series data. Slot 0 is epoch seconds (mean-bucketed
   * centroids, hence `Float64Array`); slot 1 is windowed ground speed
   * ("GPS"); slot 2 is path speed ("Path"); slot 3, when present, is
   * the recorded true-airspeed channel ("TAS"). All y-series are in
   * the user's chosen unit (km/h or mph) and bucketed against the
   * same x slice, so they line up index-for-index.
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
  rawTrack: Track,
  window: TrackWindow,
  prefs: Pick<ResolvedPreferences, 'speedUnit'>,
): SpeedSeries => {
  return useMemo((): SpeedSeries => {
    const fromIdx = window.takeoffIdx;
    const toIdx = window.landedIdx + 1;

    const padFrom = Math.max(0, fromIdx - FLIGHT_SLICE_PAD_FIXES);
    const padTo = Math.min(rawTrack.t.length, toIdx + FLIGHT_SLICE_PAD_FIXES);
    const track = sliceTrack(rawTrack, padFrom, padTo);
    const flightStart = fromIdx - padFrom;
    const flightEnd = flightStart + (toIdx - fromIdx);

    const paddedXs = track.t;
    const xs = paddedXs.slice(flightStart, flightEnd);

    const gs = computeGroundSpeed(track).slice(flightStart, flightEnd);
    const pathPadded = computePathSpeed(track);
    const pathSmoothedPadded = movingAverage(
      paddedXs,
      pathPadded,
      AIRSPEED_SMOOTHING_HALF_SECONDS,
    );
    const path = pathSmoothedPadded.slice(flightStart, flightEnd);

    const gsBucketed = bucketMean(xs, gs, SPEED_CHART_TARGET_POINTS);
    const pathBucketed = bucketMean(xs, path, SPEED_CHART_TARGET_POINTS);

    // Unit conversion runs *after* bucketing because the math is
    // linear (mean of converted = converted of mean) and post-bucket
    // arrays are an order of magnitude smaller. The whole-array
    // multiply also leaves the upstream math in km/h, which is what
    // every callsite already assumes.
    if (prefs.speedUnit === 'mph') {
      multiplyInPlace(gsBucketed.ys, KMH_TO_MPH);
      multiplyInPlace(pathBucketed.ys, KMH_TO_MPH);
    }

    if (!track.tas) {
      return { data: [gsBucketed.xs, gsBucketed.ys, pathBucketed.ys] };
    }

    const tasPadded = tasAsFloat32(track.tas);
    const tasSmoothedPadded = movingAverage(
      paddedXs,
      tasPadded,
      AIRSPEED_SMOOTHING_HALF_SECONDS,
    );
    const tas = tasSmoothedPadded.slice(flightStart, flightEnd);
    const tasBucketed = bucketMean(xs, tas, SPEED_CHART_TARGET_POINTS);
    if (prefs.speedUnit === 'mph') {
      multiplyInPlace(tasBucketed.ys, KMH_TO_MPH);
    }

    return {
      data: [gsBucketed.xs, gsBucketed.ys, pathBucketed.ys, tasBucketed.ys],
    };
  }, [rawTrack, window, prefs.speedUnit]);
};

const multiplyInPlace = (arr: Float32Array, factor: number): void => {
  for (let i = 0; i < arr.length; i++) {
    arr[i]! *= factor;
  }
};

/**
 * SoA slice that mirrors {@link Track}'s structure. Used to produce a
 * padded flight-window view of the track that `computeGroundSpeed` and
 * `computePathSpeed` can consume directly. Nullable channels stay
 * nullable; `startTime` is shifted to the new index 0.
 */
const sliceTrack = (track: Track, from: number, to: number): Track => ({
  startTime: track.t[from] ?? track.startTime,
  t: track.t.slice(from, to),
  lat: track.lat.slice(from, to),
  lng: track.lng.slice(from, to),
  alt: track.alt.slice(from, to),
  baroAlt: track.baroAlt ? track.baroAlt.slice(from, to) : null,
  tas: track.tas ? track.tas.slice(from, to) : null,
});

const tasAsFloat32 = (tas: Uint16Array): Float32Array => {
  // uPlot wants homogeneous numeric arrays; convert the integer TAS
  // channel to f32 to match the GPS / Path series.
  const out = new Float32Array(tas.length);
  for (let i = 0; i < tas.length; i++) {
    out[i] = tas[i]!;
  }
  return out;
};
