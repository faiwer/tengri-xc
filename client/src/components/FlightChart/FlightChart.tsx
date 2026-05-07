import { Segmented } from 'antd';
import { useState } from 'react';
import type { Track } from '../../track';
import type { TrackWindow } from '../../track/toPaths';
import { AltitudeChart } from './AltitudeChart';
import styles from './FlightChart.module.scss';
import { SpeedChart } from './SpeedChart';
import { VarioChart } from './VarioChart';

interface FlightChartProps {
  track: Track;
  window: TrackWindow;
}

type ChartKind = 'altitude' | 'speed' | 'vario';

/**
 * Frame around the per-flight charts. Owns the segmented control that
 * picks which view is active; each individual chart (altitude / speed /
 * vario) renders inside the same fixed-size canvas slot so switching
 * tabs doesn't shift the rest of the page layout.
 */
export function FlightChart({ track, window }: FlightChartProps) {
  const [activeKind, setActiveKind] = useState<ChartKind>('altitude');

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
          <AltitudeChart track={track} window={window} />
        )}
        {activeKind === 'speed' && <SpeedChart />}
        {activeKind === 'vario' && <VarioChart />}
      </div>
    </section>
  );
}

const SEGMENTED_OPTIONS: { label: string; value: ChartKind }[] = [
  { label: 'Altitude', value: 'altitude' },
  { label: 'Speed', value: 'speed' },
  { label: 'Vario', value: 'vario' },
];
