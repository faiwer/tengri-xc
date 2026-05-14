import { useMemo } from 'react';
import type uPlot from 'uplot';
import type { Axis, Series } from 'uplot';
import 'uplot/dist/uPlot.min.css';
import { usePreferences } from '../../core/preferences';
import type { Track } from '../../track';
import type { TrackWindow } from '../../track/toPaths';
import { varioLabel } from '../../utils/formatUnits';
import styles from './AltitudeChart.module.scss';
import { formatHourMinute } from './formatHourMinute';
import { useUPlot } from './useUPlot';
import type { HoverFractionHandler } from './useUPlot';
import { useVarioSeries } from './useVarioSeries';

interface VarioChartProps {
  track: Track;
  /** Flight portion to plot. Pre-takeoff and post-landing fixes stay off-chart. */
  window: TrackWindow;
  onHoverFractionChange?: HoverFractionHandler;
}

/**
 * Vertical velocity (m/s) over time. Single smoothed line — same ±5 s
 * centred window the map's bucket colours come from — split warm/cool at
 * the y=0 baseline so climb reads red and sink reads blue, matching the
 * map polyline's colour vocabulary.
 *
 * Both stroke and fill are vertical canvas gradients with a hard cutoff
 * at the y=0 pixel, computed via uPlot's scale-aware `valToPos`. When
 * the visible range is all-positive or all-negative the gradient
 * collapses to a single colour, which is the natural degenerate case.
 *
 * A horizontal reference line at y=0 is painted in the draw hook so the
 * sign of the signal is legible at a glance; uPlot has no first-class
 * "rule" primitive.
 */
export function VarioChart({
  track,
  window,
  onHoverFractionChange,
}: VarioChartProps) {
  const prefs = usePreferences();
  const { data } = useVarioSeries(track, window, prefs);
  const opts = useMemo(
    () => ({
      axes: [X_AXIS, buildYAxis(prefs.varioUnit)],
      series: [{}, VARIO_SERIES],
      hooks: { draw: [drawZeroRule] },
    }),
    [prefs.varioUnit],
  );
  const ref = useUPlot(data, opts, onHoverFractionChange);
  return <div ref={ref} className={styles.chart} />;
}

// Picked from the map's VARIO_COLOR_RAMP so the chart and polyline share
// a vocabulary: orange-600 ≈ +3 m/s climb, blue-500 ≈ -3 m/s sink. We use
// solid versions on the line and a 25%-alpha companion on the fill so
// the area read stays soft against the line read.
const CLIMB_STROKE = '#dc2626'; // red-600
const SINK_STROKE = '#3b82f6'; // blue-500
const CLIMB_FILL = 'rgba(220, 38, 38, 0.18)';
const SINK_FILL = 'rgba(59, 130, 246, 0.18)';
const ZERO_RULE = '#9ca3af'; // gray-400, matches the pre/post polyline tone
const AXIS_STROKE = '#6b6b73';
const AXIS_GRID = '#e3e3e7';
const SERIES_WIDTH = 1.5;

const X_AXIS: Axis = {
  stroke: AXIS_STROKE,
  grid: { stroke: AXIS_GRID },
  ticks: { stroke: AXIS_GRID },
  values: (_self, splits) =>
    splits.map((epochSeconds) => formatHourMinute(epochSeconds)),
};

const buildYAxis = (varioUnit: 'mps' | 'fpm'): Axis => {
  const suffix = varioLabel({ varioUnit });
  return {
    stroke: AXIS_STROKE,
    grid: { stroke: AXIS_GRID },
    ticks: { stroke: AXIS_GRID },
    // Splits are already in the displayed unit — `useVarioSeries` did
    // the conversion. The tick label drops the unit on m/s (kept the
    // legacy compact look) and prints it on ft/min where the magnitude
    // (3-digit ft/min vs 1-digit m/s) makes the suffix worth its width.
    values: (_self, splits) => splits.map((v) => formatVarioTick(v, suffix)),
    // fpm ticks read like "+1,000 ft/min" — 13 glyphs at the comma'd
    // four-digit max — and need real estate the m/s case doesn't.
    size: varioUnit === 'fpm' ? 104 : 72,
  };
};

/**
 * Build a vertical gradient that is `topColor` above the y=0 line and
 * `bottomColor` below it. Uses a hard colour stop at the baseline pixel
 * (no blend band) because the visual meaning is sign-of-vario, not a
 * continuous magnitude — a soft transition would suggest the wrong story.
 *
 * Falls back to a single solid colour when 0 is outside the visible
 * range, since a two-stop gradient where both stops carry the same
 * colour is wasted work.
 */
const splitAtZero =
  (topColor: string, bottomColor: string) =>
  (u: uPlot): string | CanvasGradient => {
    const yScale = u.scales['y'];
    if (!yScale || yScale.min == null || yScale.max == null) {
      return topColor;
    }

    if (yScale.min >= 0) return topColor;
    if (yScale.max <= 0) return bottomColor;

    const { top, height } = u.bbox;
    const zeroPx = u.valToPos(0, 'y', true);
    const grad = u.ctx.createLinearGradient(0, top, 0, top + height);
    // Map zero pixel into [0, 1] over the bbox so the colour stop lands
    // exactly on the baseline regardless of zoom.
    const stop = (zeroPx - top) / height;
    grad.addColorStop(0, topColor);
    grad.addColorStop(stop, topColor);
    grad.addColorStop(stop, bottomColor);
    grad.addColorStop(1, bottomColor);
    return grad;
  };

const VARIO_SERIES: Series = {
  label: 'Vario',
  stroke: splitAtZero(CLIMB_STROKE, SINK_STROKE),
  width: SERIES_WIDTH,
  fill: splitAtZero(CLIMB_FILL, SINK_FILL),
  points: { show: false },
};

const drawZeroRule = (u: uPlot): void => {
  const yScale = u.scales['y'];

  if (!yScale || yScale.min == null || yScale.max == null) {
    return;
  }

  if (yScale.min > 0 || yScale.max < 0) {
    return;
  }

  const yPx = Math.round(u.valToPos(0, 'y', true));
  const { left, top, width, height } = u.bbox;
  const ctx = u.ctx;
  ctx.save();
  ctx.beginPath();
  ctx.rect(left, top, width, height);
  ctx.clip();
  ctx.strokeStyle = ZERO_RULE;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(left, yPx + 0.5);
  ctx.lineTo(left + width, yPx + 0.5);
  ctx.stroke();
  ctx.restore();
};

function formatVarioTick(value: number, suffix: string): string {
  if (value === 0) {
    return '0';
  }
  // Compact, signed. m/s gets one decimal (range typically ±5),
  // ft/min gets whole numbers (range typically ±1000) and a suffix
  // because the magnitude alone wouldn't read as vertical velocity.
  const sign = value > 0 ? '+' : '−';
  if (suffix === 'ft/min') {
    return `${sign}${Math.round(Math.abs(value)).toLocaleString()} ${suffix}`;
  }
  return `${sign}${Math.abs(value).toFixed(1)}`;
}
