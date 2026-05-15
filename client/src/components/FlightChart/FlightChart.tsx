import { Segmented } from 'antd';
import type { ReactNode } from 'react';
import { z } from 'zod';
import { AltitudeIcon } from '../icons/AltitudeIcon';
import { SpeedIcon } from '../icons/SpeedIcon';
import { VarioIcon } from '../icons/VarioIcon';
import type { Track } from '../../track';
import type { FlightAnalysis } from '../../track/flightAnalysis';
import { useLocalStorageValue } from '../../utils/useLocalStorageValue';
import { AltitudeChart } from './AltitudeChart';
import { ChartHelpButton } from './ChartHelp';
import styles from './FlightChart.module.scss';
import { SpeedChart } from './SpeedChart';
import { CHART_LABELS, type ChartKind } from './types';
import { VarioChart } from './VarioChart';
import type { HoverFractionHandler } from './useUPlot';

interface FlightChartProps {
  track: Track;
  analysis: FlightAnalysis;
  onHoverFractionChange?: HoverFractionHandler;
  /** External map hover progress; drives the chart cursor line. */
  hoverFraction?: number | null;
}

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
      <div className={styles.controls}>
        <ChartHelpButton
          kind={activeKind}
          hasBaro={!!track.baroAlt}
          hasTas={!!track.tas}
        />
        <Segmented<ChartKind>
          className={styles.switcher}
          options={SEGMENTED_OPTIONS}
          value={activeKind}
          onChange={setActiveKind}
        />
      </div>
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

const SEGMENTED_OPTIONS: { label: ReactNode; value: ChartKind }[] = [
  {
    label: <AltitudeIcon aria-label={CHART_LABELS.altitude} />,
    value: 'altitude',
  },
  {
    label: <SpeedIcon aria-label={CHART_LABELS.speed} />,
    value: 'speed',
  },
  {
    label: <VarioIcon aria-label={CHART_LABELS.vario} />,
    value: 'vario',
  },
];

const ACTIVE_KIND_SCHEMA = z.enum(['altitude', 'speed', 'vario']);
const ACTIVE_KIND_STORAGE_OPTIONS = {
  schema: ACTIVE_KIND_SCHEMA,
  defaultValue: 'altitude' as ChartKind,
  strategy: 'initOnly' as const,
};
