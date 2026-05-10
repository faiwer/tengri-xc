import { type ReactNode } from 'react';
import type { TrackMetadata } from '../../api/tracks.io';
import { usePreferences } from '../../core/preferences';
import type { AltitudeRange } from '../../track/altitudeRange';
import type { VarioPeaks } from '../../track/varioSegments';
import { formatDuration, formatShortTime } from '../../utils/formatDateTime';
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
      <Cell label="Date">{formatVerboseDate(data.takeoffAt)}</Cell>
      <Cell label="Takeoff">{formatShortTime(data.takeoffAt, prefs)}</Cell>
      <Cell label="Landing">{formatShortTime(data.landedAt, prefs)}</Cell>
      <Cell label="Duration">
        {formatDuration(data.landedAt - data.takeoffAt)}
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

// Verbose date for the meta cell ("May 3, 2026" / "3 May 2026"). Stays
// locale-driven rather than honouring the dmy/mdy preference because
// the preference is intentionally about *short numeric* dates; the
// verbose form picks its month-name language from the locale, and
// forcing en-US/en-GB to control ordering would also force English
// month names on, say, German users. Worth revisiting once a locale
// preference exists.
const VERBOSE_DATE_FMT = new Intl.DateTimeFormat(undefined, {
  year: 'numeric',
  month: 'short',
  day: 'numeric',
});

const formatVerboseDate = (epochSeconds: number): string =>
  VERBOSE_DATE_FMT.format(new Date(epochSeconds * 1000));
