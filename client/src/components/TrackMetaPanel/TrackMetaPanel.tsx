import { type ReactNode } from 'react';
import type { TrackMetadata } from '../../api/tracks.io';
import { usePreferences } from '../../core/preferences';
import type { AltitudeRange } from '../../track/altitudeRange';
import type { VarioPeaks } from '../../track/varioSegments';
import {
  formatDuration,
  formatShortTime,
  formatVerboseDate,
} from '../../utils/formatDateTime';
import { formatAltitude, formatVario } from '../../utils/formatUnits';
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
  const prefs = usePreferences();

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
      <Cell label="Date">
        {formatVerboseDate(data.takeoffAt, data.takeoffOffset)}
      </Cell>
      <Cell label="Takeoff">
        {formatShortTime(data.takeoffAt, prefs, data.takeoffOffset)}
      </Cell>
      <Cell label="Landing">
        {formatShortTime(data.landingAt, prefs, data.landingOffset)}
      </Cell>
      <Cell label="Duration">
        {formatDuration(data.landingAt - data.takeoffAt)}
      </Cell>
      <Cell label="Best climb">
        {peaks ? formatVario(peaks.peakClimb, prefs) : '—'}
      </Cell>
      <Cell label="Best sink">
        {peaks ? formatVario(peaks.peakSink, prefs) : '—'}
      </Cell>
      <Cell label="Max alt">
        {altitudes ? formatAltitude(altitudes.maxAlt, prefs) : '—'}
      </Cell>
      <Cell label="Min alt">
        {altitudes ? formatAltitude(altitudes.minAlt, prefs) : '—'}
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
