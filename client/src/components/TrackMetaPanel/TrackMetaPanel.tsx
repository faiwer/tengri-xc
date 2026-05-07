import { useMemo, type ReactNode } from 'react';
import type { TrackMetadata } from '../../api/tracks.io';
import styles from './TrackMetaPanel.module.scss';

interface TrackMetaPanelProps {
  data: TrackMetadata;
}

export function TrackMetaPanel({ data }: TrackMetaPanelProps) {
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
