import clsx from 'clsx';
import type { ReactNode } from 'react';
import { CalendarOutlined } from '@ant-design/icons';
import type { Route, TrackMetadata } from '../api/tracks.io';
import type { FlightAnalysis } from '../track/flightAnalysis';
import { usePreferences } from '../core/preferences';
import { Flag } from '../components/Flag';
import { AltitudeIcon } from '../components/icons/AltitudeIcon';
import { RouteTypeIcon } from '../components/icons/RouteTypeIcon';
import { VarioIcon } from '../components/icons/VarioIcon';
import {
  formatDuration,
  formatShortDate,
  formatShortTime,
} from '../utils/formatDateTime';
import {
  formatAltitude,
  formatDistance,
  formatVario,
} from '../utils/formatUnits';
import styles from './MobileTrackInfo.module.scss';

interface Props {
  metadata: TrackMetadata;
  analysis: FlightAnalysis;
  selectedRoute: Route | null;
  className?: string;
}

export function MobileTrackInfo({
  metadata,
  analysis,
  selectedRoute,
  className,
}: Props) {
  const prefs = usePreferences();
  const { vario, altitudes, hasAltitudeData, hasVarioData } = analysis;
  const date = formatShortDate(
    metadata.takeoffAt,
    prefs,
    metadata.takeoffOffset,
  );
  const time = formatShortTime(
    metadata.takeoffAt,
    prefs,
    metadata.takeoffOffset,
  );
  const duration = formatDuration(metadata.landingAt - metadata.takeoffAt);

  return (
    <div className={clsx(styles.panel, className)}>
      <Row
        icon={<Flag code={metadata.pilot.country} decorative />}
        value={metadata.pilot.name}
      />
      {metadata.takeoff.name && (
        <Row
          icon={<Flag code={metadata.takeoff.country} decorative />}
          value={metadata.takeoff.name}
        />
      )}
      <Row
        icon={<CalendarOutlined className={styles.icon} />}
        value={
          <>
            {date}, {time} ({duration})
          </>
        }
      />
      {selectedRoute && (
        <Row
          icon={
            <RouteTypeIcon
              kind={selectedRoute.routeType}
              className={styles.icon}
            />
          }
          value={formatDistance(selectedRoute.distance, prefs)}
        />
      )}
      {hasVarioData && (
        <Row
          icon={<VarioIcon className={styles.icon} />}
          value={
            <>
              {formatVario(vario.peakSink, prefs)}{' '}
              <span className={styles.sep}>↔</span>{' '}
              {formatVario(vario.peakClimb, prefs)}
            </>
          }
        />
      )}
      {hasAltitudeData && altitudes && (
        <Row
          icon={<AltitudeIcon className={styles.icon} />}
          value={
            <>
              {formatAltitude(altitudes.minAlt, prefs)}{' '}
              <span className={styles.sep}>↔</span>{' '}
              {formatAltitude(altitudes.maxAlt, prefs)}
            </>
          }
        />
      )}
    </div>
  );
}

interface RowProps {
  icon: ReactNode;
  value: ReactNode;
}

function Row({ icon, value }: RowProps) {
  return (
    <div className={styles.row}>
      {icon}
      <span className={styles.value}>{value}</span>
    </div>
  );
}
