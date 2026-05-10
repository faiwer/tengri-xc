import { useMemo, type ReactNode } from 'react';
import type { TrackMetadata } from '../../api/tracks.io';
import type { AltitudeRange } from '../../track/altitudeRange';
import type { VarioPeaks } from '../../track/varioSegments';
import { formatDuration } from '../../utils/formatDateTime';
import { Flag } from '../Flag';
import styles from './TrackMetaPanel.module.scss';

interface TrackMetaPanelProps {
  data: TrackMetadata;
  /**
   * Smoothed-vario extremes over the flight window. Computed client-side
   * from the decoded track; absent until the track has loaded, so the
   * cells render `—` placeholders in the meantime.
   */
  peaks?: VarioPeaks;
  /**
   * Min and max altitude over the flight window, in metres. Same lifecycle
   * as `peaks` — absent until the track has loaded.
   */
  altitudes?: AltitudeRange;
}

export function TrackMetaPanel({
  data,
  peaks,
  altitudes,
}: TrackMetaPanelProps) {
  const takeoff = useMemo(
    () => new Date(data.takeoffAt * 1000),
    [data.takeoffAt],
  );
  const landed = useMemo(() => new Date(data.landedAt * 1000), [data.landedAt]);

  return (
    <section className={styles.panel} aria-label="Flight metadata">
      <Cell label="Pilot">
        {data.pilot.country && (
          <>
            <Flag code={data.pilot.country} />
            &nbsp;&nbsp;
          </>
        )}
        {data.pilot.name}
      </Cell>
      <Cell label="Date">{formatDate(takeoff)}</Cell>
      <Cell label="Takeoff">{formatTime(takeoff)}</Cell>
      <Cell label="Landing">{formatTime(landed)}</Cell>
      <Cell label="Duration">
        {formatDuration(data.landedAt - data.takeoffAt)}
      </Cell>
      <Cell label="Best climb">
        {peaks ? formatVario(peaks.peakClimb) : '—'}
      </Cell>
      <Cell label="Best sink">{peaks ? formatVario(peaks.peakSink) : '—'}</Cell>
      <Cell label="Max alt">
        {altitudes ? formatAltitude(altitudes.maxAlt) : '—'}
      </Cell>
      <Cell label="Min alt">
        {altitudes ? formatAltitude(altitudes.minAlt) : '—'}
      </Cell>
      <Cell label="Flight" title={data.id} mono>
        {data.id}
      </Cell>
    </section>
  );
}

interface CellProps {
  label: string;
  children: ReactNode;
  /** Render the value in a monospace face (used for ids/etags). */
  mono?: boolean;
  /** Native tooltip; useful when the value can overflow visually. */
  title?: string;
}

function Cell({ label, children, mono, title }: CellProps) {
  return (
    <div className={styles.cell} title={title}>
      <span className={styles.label}>{label}</span>
      <span className={mono ? styles.id : styles.value}>{children}</span>
    </div>
  );
}

const formatDate = (date: Date): string =>
  date.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });

const formatTime = (date: Date): string =>
  date.toLocaleTimeString(undefined, {
    hour: '2-digit',
    minute: '2-digit',
  });

const formatVario = (mps: number): string => {
  const sign = mps > 0 ? '+' : mps < 0 ? '−' : '';
  return `${sign}${Math.abs(mps).toFixed(1)} m/s`;
};

const formatAltitude = (metres: number): string =>
  `${metres.toLocaleString()} m`;
