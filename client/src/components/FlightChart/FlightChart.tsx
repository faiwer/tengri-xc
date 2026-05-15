import { Segmented } from 'antd';
import { z } from 'zod';
import type { Track } from '../../track';
import type { FlightAnalysis } from '../../track/flightAnalysis';
import { useLocalStorageValue } from '../../utils/useLocalStorageValue';
import { AltitudeChart } from './AltitudeChart';
import styles from './FlightChart.module.scss';
import { SpeedChart } from './SpeedChart';
import { VarioChart } from './VarioChart';
import type { HoverFractionHandler } from './useUPlot';

interface FlightChartProps {
  track: Track;
  analysis: FlightAnalysis;
  onHoverFractionChange?: HoverFractionHandler;
  /** External map hover progress; drives the chart cursor line. */
  hoverFraction?: number | null;
}

type ChartKind = 'altitude' | 'speed' | 'vario';

/**
 * Frame around the per-flight charts. Owns the segmented control that
 * picks which view is active; each individual chart (altitude / speed /
 * vario) renders inside the same fixed-size canvas slot so switching
 * tabs doesn't shift the rest of the page layout.
 *
 * The active tab persists across reloads via `localStorage` — pilots
 * rarely change preference between altitude / speed / vario within a
 * session, so we honour the last choice and skip the friction of
 * re-selecting it on every page load.
 */
export function FlightChart({
  track,
  analysis,
  onHoverFractionChange,
  hoverFraction,
}: FlightChartProps) {
  const [activeKind, setActiveKind] = useLocalStorageValue(
    'flight-chart-tab',
    ACTIVE_KIND_STORAGE_OPTIONS,
  );

  return (
    <section className={styles.panel} aria-label="Flight charts">
      <Segmented<ChartKind>
        className={styles.switcher}
        options={SEGMENTED_OPTIONS}
        value={activeKind}
        onChange={setActiveKind}
      />
      <div className={styles.body}>
        {activeKind === 'altitude' && (
          <AltitudeChart
            track={track}
            window={analysis.window}
            onHoverFractionChange={onHoverFractionChange}
            hoverFraction={hoverFraction}
          />
        )}
        {activeKind === 'speed' && (
          <SpeedChart
            track={track}
            analysis={analysis}
            onHoverFractionChange={onHoverFractionChange}
            hoverFraction={hoverFraction}
          />
        )}
        {activeKind === 'vario' && (
          <VarioChart
            analysis={analysis}
            onHoverFractionChange={onHoverFractionChange}
            hoverFraction={hoverFraction}
          />
        )}
      </div>
    </section>
  );
}

const SEGMENTED_OPTIONS: { label: string; value: ChartKind }[] = [
  { label: 'Altitude', value: 'altitude' },
  { label: 'Speed', value: 'speed' },
  { label: 'Vario', value: 'vario' },
];

const ACTIVE_KIND_SCHEMA = z.enum(['altitude', 'speed', 'vario']);
const ACTIVE_KIND_STORAGE_OPTIONS = {
  schema: ACTIVE_KIND_SCHEMA,
  defaultValue: 'altitude' as ChartKind,
  strategy: 'initOnly' as const,
};
