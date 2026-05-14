import { useMemo } from 'react';
import type { Axis, Series } from 'uplot';
import 'uplot/dist/uPlot.min.css';
import { usePreferences } from '../../core/preferences';
import type { Track } from '../../track';
import type { TrackWindow } from '../../track/toPaths';
import { altitudeLabel } from '../../utils/formatUnits';
import styles from './AltitudeChart.module.scss';
import { formatHourMinute } from './formatHourMinute';
import { useUPlot } from './useUPlot';
import type { HoverFractionHandler } from './useUPlot';
import { useAltitudeSeries } from './useAltitudeSeries';

interface AltitudeChartProps {
  track: Track;
  /** Flight portion to plot. Pre-takeoff and post-landing fixes stay off-chart. */
  window: TrackWindow;
  onHoverFractionChange?: HoverFractionHandler;
  hoverFraction?: number | null;
}

/**
 * Altitude over time, rendered with uPlot. Two parallel series — baro
 * (blue, area-filled) and GPS (orange, line only) — share the same x
 * axis. Baro is the primary read: smoother per-fix and the source XC
 * scoring uses for gain. GPS is overlayed because its absolute MSL is
 * unbiased (baro carries the day's QNH offset, easily ±100 m), so the
 * gap between the two lines visualises that bias directly. When the
 * track has no barometer, only the GPS series shows.
 *
 * The flight is sliced to `[takeoffIdx, landingIdx + 1)` so launch
 * jitter and post-landing handling don't pollute the y-axis range.
 *
 * uPlot's imperative lifecycle (Canvas, no JSX, mount/destroy) lives in
 * {@link useUPlot}; this component is just a config picker and a
 * container.
 */
export function AltitudeChart({
  track,
  window,
  onHoverFractionChange,
  hoverFraction,
}: AltitudeChartProps) {
  const prefs = usePreferences();
  const { data, hasBaro } = useAltitudeSeries(track, window, prefs);
  const opts = useMemo(
    () => ({
      axes: [X_AXIS, buildYAxis(prefs.units)],
      series: hasBaro ? SERIES_WITH_BARO : SERIES_GPS_ONLY,
    }),
    [hasBaro, prefs.units],
  );
  const ref = useUPlot(data, opts, onHoverFractionChange, hoverFraction);

  return <div ref={ref} className={styles.chart} />;
}

// Visual tokens. Kept here rather than in SCSS because uPlot draws to a
// canvas and reads them as JS values; mirroring them in CSS would risk
// drift. The blue and orange match the Tailwind 500-tier palette so the
// chart reads as a sibling of the metadata panel and the map polylines.
const PRIMARY_STROKE = '#3b82f6';
const PRIMARY_FILL = 'rgba(59, 130, 246, 0.12)';
const OVERLAY_STROKE = '#f97316';
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

const buildYAxis = (units: 'metric' | 'imperial'): Axis => {
  const suffix = altitudeLabel({ units });

  return {
    stroke: AXIS_STROKE,
    grid: { stroke: AXIS_GRID },
    ticks: { stroke: AXIS_GRID },
    values: (_self, splits) =>
      splits.map((v) => `${Math.round(v).toLocaleString()} ${suffix}`),
    // ft ticks reach "10,000 ft" on real flights; m ticks stay around
    // "1,234 m" so the metric case keeps the tighter axis.
    size: units === 'imperial' ? 84 : 72,
  };
};

// Two preset series arrays. uPlot's series array length must match the
// data array length, so we keep the with-baro and without-baro shapes
// separate rather than carrying a hidden slot — the cursor walks every
// y-array on hover, including hidden ones, and crashes on `null` data.
//
// With baro: blue filled "Baro" + orange "GPS" overlay drawn on top so
// the GPS line stays visible against the baro fill. Without baro: a
// single blue filled "Altitude" series — the GPS data takes the primary
// visual so the chart still has a hero line, with a label that drops
// the source distinction since there is none.
const SERIES_WITH_BARO: Series[] = [
  {},
  {
    label: 'Baro',
    stroke: PRIMARY_STROKE,
    width: SERIES_WIDTH,
    fill: PRIMARY_FILL,
    points: { show: false },
  },
  {
    label: 'GPS',
    stroke: OVERLAY_STROKE,
    width: SERIES_WIDTH,
    points: { show: false },
  },
];

const SERIES_GPS_ONLY: Series[] = [
  {},
  {
    label: 'Altitude',
    stroke: PRIMARY_STROKE,
    width: SERIES_WIDTH,
    fill: PRIMARY_FILL,
    points: { show: false },
  },
];
