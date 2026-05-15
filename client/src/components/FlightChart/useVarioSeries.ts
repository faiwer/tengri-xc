import { useMemo } from 'react';
import type { AlignedData } from 'uplot';
import type { ResolvedPreferences } from '../../core/preferences';
import type { FlightAnalysis } from '../../track/flightAnalysis';
import { bucketMean } from '../../utils/bucketMean';
import { MPS_TO_FPM } from '../../utils/formatUnits';

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
   * vertical velocity (positive = climb), bucket-averaged. Y values
   * are in m/s or ft/min depending on the supplied preferences.
   */
  data: AlignedData;
}

/**
 * Build the uPlot data arrays for {@link VarioChart}, sliced to the
 * flight window and mean-bucketed to at most {@link VARIO_CHART_TARGET_POINTS}.
 *
 * `computeVario` runs over the full track because its centred ±5 s window
 * needs neighbours that may live just outside `[takeoffIdx, landingIdx + 1)`;
 * we then slice the result, convert to the user's unit, and bucket. The
 * unit conversion happens *before* bucketing so the bucketed averages
 * land in the displayed unit and the y-axis tick formatter only needs
 * to print the suffix.
 */
export const useVarioSeries = (
  analysis: FlightAnalysis,
  prefs: Pick<ResolvedPreferences, 'varioUnit'>,
): VarioSeries => {
  return useMemo((): VarioSeries => {
    const { track, window, metrics } = analysis;
    const fromIdx = window.takeoffIdx;
    const toIdx = window.landingIdx + 1;
    const xs = track.t.slice(fromIdx, toIdx);
    const vario = metrics.vario.slice(fromIdx, toIdx);
    if (prefs.varioUnit === 'fpm') {
      for (let i = 0; i < vario.length; i++) {
        vario[i] *= MPS_TO_FPM;
      }
    }
    const bucketed = bucketMean(xs, vario, VARIO_CHART_TARGET_POINTS);
    return { data: [bucketed.xs, bucketed.ys] };
  }, [analysis, prefs.varioUnit]);
};
