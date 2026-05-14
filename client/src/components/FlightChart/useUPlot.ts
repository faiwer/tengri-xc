import { useEffect, useRef, type RefObject } from 'react';
import uPlot, { type AlignedData, type Options } from 'uplot';

export type HoverFractionHandler = (fraction: number | null) => void;

/**
 * Mount and manage a uPlot instance against a container div.
 *
 * Returns a ref to attach to the container. The hook owns the full
 * uPlot lifecycle: construction on mount and on `data` / `opts`
 * change, ResizeObserver-driven `setSize`, and destruction on unmount
 * or before each rebuild.
 *
 * The hook supplies the chart conventions shared across the FlightChart
 * panel — hover-only cursor, time x-scale, auto y-scale, hidden legend
 * — and shallow-merges `opts` over them, so callers only specify what
 * varies (axes, series, and whatever else they want to override).
 *
 * uPlot reads its width/height once at construction, so any change in
 * `data` or `opts` triggers a full teardown-and-rebuild rather than an
 * in-place patch. Construction is sub-millisecond on the data sizes
 * the FlightChart panel hands it, so this is cheaper and simpler than
 * maintaining diff logic.
 *
 * Memoise `opts` at the call site (e.g. via the same array of static
 * presets the original components used) — every new reference triggers
 * a rebuild.
 */
export const useUPlot = (
  data: AlignedData,
  opts: Pick<Options, 'axes' | 'series'> & Partial<Options>,
  onHoverFractionChange?: HoverFractionHandler,
): RefObject<HTMLDivElement | null> => {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    const setCursorHooks = opts.hooks?.setCursor ?? [];
    const hooks: Options['hooks'] = {
      ...opts.hooks,
      setCursor: onHoverFractionChange
        ? [
            ...setCursorHooks,
            (chart) => {
              onHoverFractionChange(geChartHoverFraction(chart, data));
            },
          ]
        : setCursorHooks,
    };

    const merged: Options = {
      width: container.clientWidth,
      height: container.clientHeight,
      cursor: {
        show: true,
        x: true,
        y: false,
        drag: { x: false, y: false },
      },
      scales: {
        x: { time: true },
        y: { auto: true },
      },
      legend: { show: false },
      ...opts,
      hooks,
    };

    const chart = new uPlot(merged, data, container);
    const clearHover = () => {
      onHoverFractionChange?.(null);
    };
    container.addEventListener('mouseleave', clearHover);

    const resize = new ResizeObserver(() => {
      chart.setSize({
        width: container.clientWidth,
        height: container.clientHeight,
      });
    });
    resize.observe(container);

    return () => {
      clearHover();
      container.removeEventListener('mouseleave', clearHover);
      resize.disconnect();
      chart.destroy();
    };
  }, [data, opts, onHoverFractionChange]);

  return containerRef;
};

const geChartHoverFraction = (
  chart: uPlot,
  data: AlignedData,
): number | null => {
  const hoverIdx = chart.cursor.idx;
  const timeSeries = data[0];

  if (hoverIdx == null || hoverIdx < 0 || hoverIdx >= timeSeries.length) {
    return null;
  }

  if (timeSeries.length < 2) {
    return null;
  }

  const first = Number(timeSeries[0]);
  const last = Number(timeSeries[timeSeries.length - 1]);
  const current = Number(timeSeries[hoverIdx]);
  const span = last - first;

  if (span <= 0) {
    return null;
  }

  return clamp((current - first) / span, 0, 1);
};

const clamp = (value: number, min: number, max: number): number =>
  value < min ? min : value > max ? max : value;
