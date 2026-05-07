import { useMemo } from 'react';
import type { AlignedData } from 'uplot';
import type { Track } from '../../track';
import type { TrackWindow } from '../../track/toPaths';

export interface AltitudeSeries {
  /**
   * uPlot-shaped series data: `[xs, ...yArrays]`. Slot 0 is epoch
   * seconds. Slot 1 is the primary altitude (baro when present, GPS
   * otherwise). Slot 2, when present, is the GPS overlay.
   */
  data: AlignedData;
  hasBaro: boolean;
}

/**
 * Build the uPlot data arrays for {@link AltitudeChart}, sliced to the
 * flight window so launch jitter and post-landing handling don't pollute
 * the y-axis range.
 *
 * The shape varies by data availability:
 * - With baro: `[xs, baroAlt, gpsAlt]` — both lines render, baro as the
 *   blue filled primary, GPS as an orange overlay.
 * - Without baro: `[xs, gpsAlt]` — single hero line, styled blue+filled.
 *
 * Hidden-but-allocated series are *not* an option here: uPlot's cursor
 * walks every y-array on hover, so the series array length must match
 * the data array length. The {@link hasBaro} flag lets the consumer
 * configure styling without re-deriving from `data.length`.
 */
export const useAltitudeSeries = (
  track: Track,
  window: TrackWindow,
): AltitudeSeries => {
  return useMemo(() => {
    const fromIdx = window.takeoffIdx;
    const toIdx = window.landedIdx + 1;
    const length = toIdx - fromIdx;
    const xs = track.t.subarray(fromIdx, toIdx);

    const gpsAlt = new Float32Array(length);
    for (let i = 0; i < length; i++) {
      gpsAlt[i] = track.alt[fromIdx + i]! / 10;
    }

    if (!track.baroAlt) {
      return { data: [xs, gpsAlt], hasBaro: false };
    }

    const baroAlt = new Float32Array(length);
    for (let i = 0; i < length; i++) {
      baroAlt[i] = track.baroAlt[fromIdx + i]! / 10;
    }

    return { data: [xs, baroAlt, gpsAlt], hasBaro: true };
  }, [track, window]);
};
