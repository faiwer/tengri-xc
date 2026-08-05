import { type ReactNode } from 'react';
import type { Route, TrackMetadata } from '../../api/tracks.io';
import { RouteSwitcher } from './RouteSwitcher';
import { usePreferences } from '../../core/preferences';
import type { AltitudeRange } from '../../track/altitudeRange';
import type { VarioPeaks } from '../../track/varioSegments';
import clsx from 'clsx';
import {
  formatDuration,
  formatShortTime,
  formatVerboseDate,
} from '../../utils/formatDateTime';
import {
  formatAltitude,
  formatDistance,
  formatVario,
} from '../../utils/formatUnits';
import { TextWithIcon } from '../TextWithIcon';
import { LandingLabel } from './LandingLabel';
import { FlightActionsMenu } from './FlightActionsMenu';
import styles from './TrackMetaPanel.module.scss';
import { GliderKindIcon } from '../icons';
import { useIdentity, hasPermission, Permissions } from '../../core/identity';

interface TrackMetaPanelProps {
  data: TrackMetadata;
  selectedRoute: Route | null;
  onRouteSelect: (route: Route) => void;
  /** `undefined` until track analysis has loaded. */
  hasAltitudeData?: boolean;
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
  altitudes?: AltitudeRange | null;
}

export function TrackMetaPanel({
  data,
  selectedRoute,
  onRouteSelect,
  hasAltitudeData,
  peaks,
  altitudes,
}: TrackMetaPanelProps) {
  const prefs = usePreferences();
  const { me } = useIdentity();
  const showAltitudeFields = hasAltitudeData !== false;
  const canManage =
    me != null &&
    (hasPermission(me, Permissions.MANAGE_TRACKS) || me.id === data.pilot.id);

  return (
    <section className={styles.panel} aria-label="Flight metadata">
      <div className={clsx(styles.header, canManage && styles.withMenu)}>
        <Cell>{formatVerboseDate(data.takeoffAt, data.takeoffOffset)}</Cell>
        {canManage && (
          <FlightActionsMenu
            flightId={data.id}
            anchorClassName={styles.menuAnchor}
          />
        )}
      </div>
      <Cell>
        <TextWithIcon flag={data.pilot.country} text={data.pilot.name} />
      </Cell>
      {data.takeoff.name && (
        <Cell>
          <TextWithIcon flag={data.takeoff.country} text={data.takeoff.name} />
        </Cell>
      )}
      <Cell label="Glider">
        <TextWithIcon
          layout="reverse"
          icon={<GliderKindIcon kind={data.glider.kind} tooltip="singular" />}
          text={data.glider.brandName + ' ' + data.glider.modelName}
        />
      </Cell>
      <Cell label="Takeoff">
        {formatShortTime(data.takeoffAt, prefs, data.takeoffOffset)}
      </Cell>
      <Cell label={<LandingLabel data={data} />}>
        {formatShortTime(data.landingAt, prefs, data.landingOffset)}
      </Cell>
      <Cell label="Duration">
        {formatDuration(data.landingAt - data.takeoffAt)}
      </Cell>
      <Cell label="Route">
        <span className={styles.routeValue}>
          {selectedRoute
            ? `${formatDistance(selectedRoute.distance, prefs)}, score: ${selectedRoute.score.toFixed(2)}`
            : '—'}
          <RouteSwitcher
            routes={data.routes}
            selectedRoute={selectedRoute}
            onSelect={onRouteSelect}
          />
        </span>
      </Cell>
      {showAltitudeFields && (
        <>
          <Cell label="Best sink & climb">
            {peaks
              ? `${formatVario(peaks.peakSink, prefs)} ↔ ${formatVario(peaks.peakClimb, prefs)}`
              : '—'}
          </Cell>
          <Cell label="Min & max alt">
            {altitudes
              ? `${formatAltitude(altitudes.minAlt, prefs)} ↔ ${formatAltitude(altitudes.maxAlt, prefs)}`
              : '—'}
          </Cell>
        </>
      )}
    </section>
  );
}

interface CellProps {
  /** Optional row label. Omit for title-less rows (e.g. pilot, takeoff site). */
  label?: ReactNode;
  children: ReactNode;
  /** Render the value in a monospace face (used for ids/etags). */
  mono?: boolean;
  /** Native tooltip; useful when the value can overflow visually. */
  title?: string;
}

function Cell({ label, children, mono, title }: CellProps) {
  return (
    <div className={styles.cell} title={title}>
      {!!label && <span className={styles.label}>{label}</span>}
      <span className={mono ? styles.id : styles.value}>{children}</span>
    </div>
  );
}
