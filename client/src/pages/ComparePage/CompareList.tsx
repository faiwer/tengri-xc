import { Fragment } from 'react';
import { RouteTypeIcon } from '../../components/icons/RouteTypeIcon';
import { TextWithIcon } from '../../components/TextWithIcon';
import { usePreferences } from '../../core/preferences';
import { formatDuration, formatVerboseDate } from '../../utils/formatDateTime';
import { formatDistance } from '../../utils/formatUnits';
import type { CompareFlight } from './useComparePageData';
import styles from './ComparePage.module.scss';

interface CompareListProps {
  flights: CompareFlight[];
}

export function CompareList({ flights }: CompareListProps) {
  const prefs = usePreferences();

  return (
    <section className={styles.list} aria-label="Compared flights">
      {flights.map((flight, index) => (
        <Fragment key={`${flight.id}-${index}`}>
          <FlightSummary flight={flight} prefs={prefs} />
          {index < flights.length - 1 && <hr className={styles.separator} />}
        </Fragment>
      ))}
    </section>
  );
}

interface FlightSummaryProps {
  flight: CompareFlight;
  prefs: ReturnType<typeof usePreferences>;
}

function FlightSummary({ flight, prefs }: FlightSummaryProps) {
  const { color, metadata } = flight;

  return (
    <div className={styles.flight}>
      <div className={styles.row}>
        <span className={styles.circle} style={{ background: color }} />
        {metadata.status === 'ok' ? (
          <TextWithIcon
            flag={metadata.data.pilot.country}
            text={metadata.data.pilot.name}
          />
        ) : (
          <span className={styles.muted}>
            {metadata.status === 'error' ? 'Failed to load' : flight.id}
          </span>
        )}
      </div>
      {metadata.status === 'ok' && (
        <div className={styles.meta}>
          <span>
            {formatDuration(metadata.data.landingAt - metadata.data.takeoffAt)},
          </span>
          {metadata.data.mainRoute && (
            <span className={styles.route}>
              <RouteTypeIcon kind={metadata.data.mainRoute.routeType} />{' '}
              {formatDistance(metadata.data.mainRoute.distance, prefs)}
            </span>
          )}
        </div>
      )}
      {metadata.status === 'ok' && (
        <div className={styles.meta}>
          {formatVerboseDate(
            metadata.data.takeoffAt,
            metadata.data.takeoffOffset,
          )}
        </div>
      )}
    </div>
  );
}
