import { useMemo, type ReactNode } from 'react';
import type { TrackMetadata } from '../../api/tracks.io';
import type { AltitudeRange } from '../../track/altitudeRange';
import type { VarioPeaks } from '../../track/varioSegments';
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
    () => new Date(data.takeoff_at * 1000),
    [data.takeoff_at],
  );
  const landed = useMemo(
    () => new Date(data.landed_at * 1000),
    [data.landed_at],
  );

  return (
    <section className={styles.panel} aria-label="Flight metadata">
      <Cell label="Pilot">{data.pilot.name}</Cell>
      <Cell label="Date">{formatDate(takeoff)}</Cell>
      <Cell label="Takeoff">{formatTime(takeoff)}</Cell>
      <Cell label="Landing">{formatTime(landed)}</Cell>
      <Cell label="Duration">
        {formatDuration(data.landed_at - data.takeoff_at)}
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

/**
 * Formats a flight duration in seconds as `2h 29m` (or `47m` under an hour).
 * Sub-minute precision is meaningless at the panel scale; truncate to the
 * full minute. Non-positive inputs render as `—` rather than `0m`, since
 * they only happen when the track's takeoff/landing aren't yet known.
 */
const formatDuration = (totalSeconds: number): string => {
  if (totalSeconds <= 0) {
    return '—';
  }

  const totalMinutes = Math.floor(totalSeconds / 60);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours === 0) {
    return `${minutes}m`;
  }

  return `${hours}h ${minutes.toString().padStart(2, '0')}m`;
};
