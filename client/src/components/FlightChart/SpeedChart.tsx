import { useMemo } from 'react';
import type { Axis, Series } from 'uplot';
import 'uplot/dist/uPlot.min.css';
import { usePreferences } from '../../core/preferences';
import type { Track } from '../../track';
import type { FlightAnalysis } from '../../track/flightAnalysis';
import { speedLabel } from '../../utils/formatUnits';
import { formatHourMinute } from './formatHourMinute';
import styles from './SpeedChart.module.scss';
import { useSpeedSeries } from './useSpeedSeries';
import { useUPlot } from './useUPlot';
import type { HoverFractionHandler } from './useUPlot';

interface SpeedChartProps {
  track: Track;
  analysis: FlightAnalysis;
  onHoverFractionChange?: HoverFractionHandler;
  hoverFraction?: number | null;
}

/**
 * Speed over time. Three series, each named after its source rather
 * than the physical quantity it most closely measures, because the
 * mapping shifts depending on flight regime:
 *
 * - **GPS** (blue, area-filled) — displacement across a centred ±30 s
 *   window divided by elapsed time. The pilot's actual speed across
 *   the ground; collapses toward wind drift inside thermals.
 * - **Path** (light violet) — sum of per-leg chord lengths divided by
 *   Δt, then ±30 s smoothed. Inside a turn this converges to airspeed
 *   (wind cancels around the circle); on a long straight glide it
 *   equals GPS speed, since path and displacement coincide there.
 * - **TAS** (orange) — the instrument's recorded true airspeed,
 *   shown when the source IGC carries a TAS column. Same ±30 s
 *   smoothing for parity with the Path line.
 *
 * On TAS-equipped tracks the agreement (or disagreement) of Path and
 * TAS is itself a useful read on instrument calibration: tight overlap
 * inside thermals confirms the TAS sensor is honest; persistent offsets
 * point at calibration error.
 *
 * uPlot's imperative lifecycle (Canvas, no JSX, mount/destroy) lives in
 * {@link useUPlot}; this component is just a config picker and a
 * container.
 */
export function SpeedChart({
  track,
  analysis,
  onHoverFractionChange,
  hoverFraction,
}: SpeedChartProps) {
  const prefs = usePreferences();
  const { data } = useSpeedSeries(analysis, prefs);
  const hasTas = !!track.tas;
  const opts = useMemo(
    () => ({
      axes: [X_AXIS, buildYAxis(prefs.speedUnit)],
      series: hasTas
        ? [{}, GPS_SERIES, PATH_SERIES, TAS_SERIES]
        : [{}, GPS_SERIES, PATH_SERIES],
    }),
    [hasTas, prefs.speedUnit],
  );
  const ref = useUPlot(data, opts, onHoverFractionChange, hoverFraction);

  return <div ref={ref} className={styles.chart} />;
}

// Visual tokens kept in sync with AltitudeChart so the two charts read
// as siblings — same blue / orange palette and same axis greys.
const GPS_STROKE = '#3b82f6';
const GPS_FILL = 'rgba(59, 130, 246, 0.12)';
const TAS_STROKE = '#f97316';
// Tailwind violet-400. Light enough to read as "softer / derived" next
// to the saturated blue and orange, distinct enough not to be confused
// with either at chart density.
const PATH_STROKE = '#a78bfa';
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

const buildYAxis = (speedUnit: 'kmh' | 'mph'): Axis => {
  const suffix = speedLabel({ speedUnit });
  return {
    stroke: AXIS_STROKE,
    grid: { stroke: AXIS_GRID },
    ticks: { stroke: AXIS_GRID },
    values: (_self, splits) => splits.map((v) => `${Math.round(v)} ${suffix}`),
    size: 80,
  };
};

const GPS_SERIES: Series = {
  label: 'GPS',
  stroke: GPS_STROKE,
  width: SERIES_WIDTH,
  fill: GPS_FILL,
  points: { show: false },
};

const PATH_SERIES: Series = {
  label: 'Path',
  stroke: PATH_STROKE,
  width: SERIES_WIDTH,
  points: { show: false },
};

const TAS_SERIES: Series = {
  label: 'TAS',
  stroke: TAS_STROKE,
  width: SERIES_WIDTH,
  points: { show: false },
};
