import { Skeleton } from 'antd';
import { useMemo } from 'react';
import type { RouteType, TrackListItem } from '../../api/tracks.io';
import { Flag } from '../../components/Flag';
import { LoadError } from '../../components/LoadError';
import { PageLayout } from '../../components/PageLayout';
import { TrackRow, type TrackRowCell } from '../../components/TrackRow';
import { useErrorToast } from '../../core/hooks';
import {
  usePreferences,
  type ResolvedPreferences,
} from '../../core/preferences';
import { formatDuration, formatShortDate } from '../../utils/formatDateTime';
import { formatDistance } from '../../utils/formatUnits';
import styles from './TracksPage.module.scss';
import { useScrollSentinel } from './useScrollSentinel';
import { useTracksFeed } from './useTracksFeed';
import { RouteTypeIcon } from '../../components/icons/RouteTypeIcon';

/**
 * Global tracks feed: cursor-paginated, infinite-scroll on window
 * scroll. The visible table is hand-rolled around the reusable
 */
export function TracksPage() {
  const feed = useTracksFeed();
  const onSentinelRef = useScrollSentinel(feed.loadMore);
  const prefs = usePreferences();

  const rows = useMemo(
    () =>
      (feed.items ?? []).map((item, index) => ({
        item,
        cells: buildHomeRowCells(item, index + 1, prefs),
      })),
    [feed.items, prefs],
  );

  const isEmpty = feed.items?.length === 0 && !feed.isLoading;
  const hasInlineError = feed.error !== null && feed.items === null;
  useErrorToast(feed.error, { title: "Couldn't load flights" });

  return (
    <PageLayout>
      <table className={styles.table}>
        <thead>
          <tr>
            <th className={`${styles.colIdx} ${styles.alignRight}`}>#</th>
            <th className={styles.colDate}>Date</th>
            <th>Pilot</th>
            <th className={`${styles.colDuration} ${styles.alignRight}`}>
              Duration
            </th>
            <th className={styles.colScore}>Score</th>
            <th className={styles.colDist}>Distance</th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ item, cells }) => (
            <TrackRow key={item.track.id} item={item} cells={cells} />
          ))}
          {feed.isLoading && <SkeletonRows colSpan={6} />}
        </tbody>
      </table>

      {isEmpty && <p className={styles.empty}>No flights yet.</p>}

      {hasInlineError && (
        <LoadError
          title="Couldn't load flights"
          error={feed.error}
          onRetry={feed.retry}
        />
      )}

      {!feed.isLoading && !feed.completed && (
        <div ref={onSentinelRef} className={styles.sentinel} aria-hidden />
      )}
    </PageLayout>
  );
}

function buildHomeRowCells(
  item: TrackListItem,
  rowNumber: number,
  prefs: ResolvedPreferences,
): TrackRowCell[] {
  return [
    {
      key: 'idx',
      content: rowNumber,
      align: 'right',
      className: styles.colIdx,
    },
    {
      key: 'date',
      content: formatShortDate(
        item.track.takeoffAt,
        prefs,
        item.track.takeoffOffset,
      ),
      muted: true,
      className: styles.colDate,
    },
    {
      key: 'pilot',
      content: (
        <>
          {item.pilot.country && (
            <>
              <Flag code={item.pilot.country} />
              &nbsp;&nbsp;
            </>
          )}
          {item.pilot.name}
        </>
      ),
    },
    {
      key: 'duration',
      content: formatDuration(item.track.duration),
      align: 'right',
      className: styles.colDuration,
    },
    {
      key: 'score',
      content: formatScore(item.track.mainRouteType, item.track.mainScore),
      align: 'left',
      muted: item.track.mainScore == null,
      className: styles.colScore,
    },
    {
      key: 'dist',
      content:
        item.track.mainDistance != null
          ? formatDistance(item.track.mainDistance, prefs)
          : '—',
      align: 'left',
      muted: item.track.mainDistance == null,
      className: styles.colDist,
    },
  ];
}

const formatScore = (
  routeType: RouteType | null,
  score: number | null | undefined,
): React.ReactNode => {
  if (routeType == null || score == null) {
    return '—';
  }

  return (
    <>
      <RouteTypeIcon kind={routeType} /> {score.toFixed(2)}
    </>
  );
};

function SkeletonRows({ colSpan }: { colSpan: number }) {
  return (
    <>
      {Array.from({ length: LOADING_SKELETON_COUNT }, (_, i) => (
        <tr key={`sk-${i}`} className={styles.skeletonRow}>
          <td colSpan={colSpan}>
            <Skeleton.Input active block size="small" />
          </td>
        </tr>
      ))}
    </>
  );
}

const LOADING_SKELETON_COUNT = 8;
