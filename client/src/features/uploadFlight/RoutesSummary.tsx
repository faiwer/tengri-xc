import { HourglassOutlined } from '@ant-design/icons';
import { Tooltip } from 'antd';
import { useMemo } from 'react';
import type { TrackPeekMetadata } from '../../api/tracks.io';
import { RouteTypeIcon } from '../../components/icons/RouteTypeIcon';
import { usePreferences } from '../../core/preferences';
import { formatDuration } from '../../utils/formatDateTime';
import { formatDistance } from '../../utils/formatUnits';
import styles from './RoutesSummary.module.scss';

export function RoutesSummary({ metadata }: { metadata: TrackPeekMetadata }) {
  const prefs = usePreferences();
  const routes = useMemo(
    () => [...metadata.routes].sort((a, b) => b.score - a.score),
    [metadata.routes],
  );
  const duration = metadata.landingAt - metadata.takeoffAt;

  return (
    <div className={styles.summary}>
      {routes.map((route) => (
        <Tooltip key={route.id} title="Approximated distance">
          <div className={styles.summaryItem}>
            <RouteTypeIcon
              kind={route.routeType}
              className={styles.summaryIcon}
            />
            <span className={styles.summaryValue}>
              {formatDistance(route.distance, prefs)}
            </span>
          </div>
        </Tooltip>
      ))}
      <div className={styles.summaryItem}>
        <HourglassOutlined className={styles.summaryIcon} />
        <span className={styles.summaryValue}>{formatDuration(duration)}</span>
      </div>
    </div>
  );
}
