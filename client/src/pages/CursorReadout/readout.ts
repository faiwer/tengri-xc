import { usePreferences } from '../../core/preferences';
import type { FlightAnalysis } from '../../track/flightAnalysis';
import { formatShortTimeWithSeconds } from '../../utils/formatDateTime';
import {
  formatAltitude,
  formatSpeed,
  formatVario,
  MPS_TO_KMH,
} from '../../utils/formatUnits';
import type { CursorReadoutValue, CursorReadoutWidths } from './types';

export const buildCursorReadout = (
  analysis: FlightAnalysis,
  idx: number,
  prefs: ReturnType<typeof usePreferences>,
): CursorReadoutValue => {
  const { track, metrics } = analysis;
  return {
    time: formatShortTimeWithSeconds(
      track.t[idx]!,
      prefs,
      analysis.timeOffsetSeconds,
    ),
    gps: formatAltitude(track.alt[idx] / 10, prefs),
    baroAlt: track.baroAlt
      ? formatAltitude(track.baroAlt[idx] / 10, prefs)
      : null,
    pathSpeed: formatSpeed(metrics.pathSpeed[idx] / MPS_TO_KMH, prefs),
    tas: metrics.tas ? formatSpeed(metrics.tas[idx] / MPS_TO_KMH, prefs) : null,
    vario: formatVario(metrics.vario[idx], prefs),
    speed: formatSpeed(metrics.speed[idx] / MPS_TO_KMH, prefs),
  };
};

export const buildCursorReadoutWidths = (
  analysis: FlightAnalysis,
  prefs: ReturnType<typeof usePreferences>,
): CursorReadoutWidths => {
  const { track, metrics, window, altitudes, vario } = analysis;
  const fromIdx = window.takeoffIdx;
  const toIdx = window.landingIdx + 1;
  const baroAltRange = track.baroAlt
    ? range(track.baroAlt, fromIdx, toIdx, (dm) => dm / 10)
    : null;
  const maxSpeed = max(metrics.speed, fromIdx, toIdx);
  const maxPathSpeed = max(metrics.pathSpeed, fromIdx, toIdx);
  const maxTas = metrics.tas ? max(metrics.tas, fromIdx, toIdx) : null;

  return {
    time: Math.max(
      formatShortTimeWithSeconds(
        track.t[fromIdx] ?? 0,
        prefs,
        analysis.timeOffsetSeconds,
      ).length,
      formatShortTimeWithSeconds(
        track.t[window.landingIdx] ?? 0,
        prefs,
        analysis.timeOffsetSeconds,
      ).length,
    ),
    gps: Math.max(
      formatAltitude(altitudes.minAlt, prefs).length,
      formatAltitude(altitudes.maxAlt, prefs).length,
    ),
    baroAlt: baroAltRange
      ? Math.max(
          formatAltitude(baroAltRange.min, prefs).length,
          formatAltitude(baroAltRange.max, prefs).length,
        )
      : undefined,
    vario: Math.max(
      formatVario(vario.peakSink, prefs).length,
      formatVario(vario.peakClimb, prefs).length,
    ),
    // Ground-speed metrics are stored as km/h; formatSpeed takes m/s.
    speed: formatSpeed(maxSpeed / MPS_TO_KMH, prefs).length,
    pathSpeed: formatSpeed(maxPathSpeed / MPS_TO_KMH, prefs).length,
    tas:
      maxTas === null
        ? undefined
        : formatSpeed(maxTas / MPS_TO_KMH, prefs).length,
  };
};

const max = (
  values: ArrayLike<number>,
  fromIdx: number,
  toIdx: number,
): number => {
  let result = 0;
  for (let idx = fromIdx; idx < toIdx; idx++) {
    const value = values[idx] ?? 0;
    if (value > result) {
      result = value;
    }
  }
  return result;
};

const range = (
  values: ArrayLike<number>,
  fromIdx: number,
  toIdx: number,
  convert: (value: number) => number,
): { min: number; max: number } => {
  if (fromIdx >= toIdx) {
    return { min: 0, max: 0 };
  }

  let minValue = convert(values[fromIdx] ?? 0);
  let maxValue = minValue;
  for (let idx = fromIdx + 1; idx < toIdx; idx++) {
    const value = convert(values[idx] ?? 0);
    if (value < minValue) {
      minValue = value;
    } else if (value > maxValue) {
      maxValue = value;
    }
  }

  return { min: minValue, max: maxValue };
};
