import { useEffect, useRef } from 'react';
import uPlot, { type Axis, type Options, type Series } from 'uplot';
import 'uplot/dist/uPlot.min.css';
import type { Track } from '../../track';
import type { TrackWindow } from '../../track/toPaths';
import styles from './AltitudeChart.module.scss';
import { useAltitudeSeries } from './useAltitudeSeries';

interface AltitudeChartProps {
  track: Track;
  /** Flight portion to plot. Pre-takeoff and post-landing fixes stay off-chart. */
  window: TrackWindow;
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
 * The flight is sliced to `[takeoffIdx, landedIdx + 1)` so launch
 * jitter and post-landing handling don't pollute the y-axis range.
 *
 * uPlot is imperative (Canvas, no JSX): we mount a div, hand it to
 * uPlot in a layout-effect, and resize/destroy in lifecycle hooks. The
 * chart is rebuilt on track change rather than diffed via `setData` —
 * uPlot construction is sub-millisecond and recreation keeps the
 * lifecycle trivially correct.
 */
export function AltitudeChart({ track, window }: AltitudeChartProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<uPlot | null>(null);
  const { data, hasBaro } = useAltitudeSeries(track, window);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    const opts: Options = {
      width: container.clientWidth,
      height: container.clientHeight,
      // uPlot reads its dimensions once at construction; we keep them in
      // sync via setSize on resize and on every cleanup-then-create cycle.
      cursor: { drag: { setScale: false } },
      scales: {
        x: { time: true },
        y: { auto: true },
      },
      axes: [X_AXIS, Y_AXIS],
      series: hasBaro ? SERIES_WITH_BARO : SERIES_GPS_ONLY,
      legend: { show: false },
    };

    const chart = new uPlot(opts, data, container);
    chartRef.current = chart;

    const resize = new ResizeObserver(() => {
      chart.setSize({
        width: container.clientWidth,
        height: container.clientHeight,
      });
    });
    resize.observe(container);

    return () => {
      resize.disconnect();
      chart.destroy();
      chartRef.current = null;
    };
  }, [data, hasBaro]);

  return <div ref={containerRef} className={styles.chart} />;
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

// Tick labels need to read as wide as possible for many ticks, so we
// strip the AM/PM suffix `Intl.DateTimeFormat` insists on for 12h
// locales.
const HOUR_MINUTE_FORMATTER = new Intl.DateTimeFormat(undefined, {
  hour: 'numeric',
  minute: '2-digit',
});

const formatHourMinute = (epochSeconds: number): string =>
  HOUR_MINUTE_FORMATTER.formatToParts(new Date(epochSeconds * 1000))
    .filter(
      (part) =>
        part.type !== 'dayPeriod' &&
        !(part.type === 'literal' && part.value.trim() === ''),
    )
    .map((part) => part.value)
    .join('');

const X_AXIS: Axis = {
  stroke: AXIS_STROKE,
  grid: { stroke: AXIS_GRID },
  ticks: { stroke: AXIS_GRID },
  values: (_self, splits) =>
    splits.map((epochSeconds) => formatHourMinute(epochSeconds)),
};

const Y_AXIS: Axis = {
  stroke: AXIS_STROKE,
  grid: { stroke: AXIS_GRID },
  ticks: { stroke: AXIS_GRID },
  values: (_self, splits) => splits.map((m) => `${Math.round(m)} m`),
  size: 64,
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
