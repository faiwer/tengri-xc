import { useEffect, useRef, type RefObject } from 'react';
import uPlot, { type AlignedData, type Options } from 'uplot';

/**
 * Mount and manage a uPlot instance against a container div.
 *
 * Returns a ref to attach to the container. The hook owns the full
 * uPlot lifecycle: construction on mount and on `data` / `opts`
 * change, ResizeObserver-driven `setSize`, and destruction on unmount
 * or before each rebuild.
 *
 * The hook supplies the chart conventions shared across the FlightChart
 * panel — non-zooming cursor, time x-scale, auto y-scale, hidden legend
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
): RefObject<HTMLDivElement | null> => {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    const merged: Options = {
      width: container.clientWidth,
      height: container.clientHeight,
      cursor: { drag: { setScale: false } },
      scales: {
        x: { time: true },
        y: { auto: true },
      },
      legend: { show: false },
      ...opts,
    };

    const chart = new uPlot(merged, data, container);

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
    };
  }, [data, opts]);

  return containerRef;
};
