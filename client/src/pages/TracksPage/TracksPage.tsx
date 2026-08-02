import { Skeleton } from 'antd';
import { Fragment, useMemo } from 'react';
import type { RouteType, TrackListItem } from '../../api/tracks.io';
import { Flag } from '../../components/Flag';
import { LoadError } from '../../components/LoadError';
import { PageLayout } from '../../components/PageLayout';
import { TrackRow, type TrackRowCell } from '../../components/TrackRow';
import { useAsyncEffect, useErrorToast } from '../../core/hooks';
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

  // Restore the scroll position captured when we last left for a flight page.
  // Runs once on mount; a hydrated feed renders its rows synchronously, so the
  // table is already tall enough to scroll to `initialScrollTop`.
  useAsyncEffect(() => {
    if (feed.initialScrollTop > 0) {
      window.scrollTo(0, feed.initialScrollTop);
    }
  }, []);

  const rows = useMemo(
    () =>
      (feed.items ?? []).map((item, index) => ({
        item,
        cells: buildHomeRowCells(item, index + 1, prefs),
        date: formatShortDate(
          item.track.takeoffAt,
          prefs,
          item.track.takeoffOffset,
        ),
      })),
    [feed.items, prefs],
  );

  const isEmpty = feed.items?.length === 0 && !feed.isLoading;
  const hasInlineError = feed.error !== null && feed.items === null;
  useErrorToast(feed.error, { title: "Couldn't load flights" });

  return (
    <PageLayout>
      <table
        className={styles.table}
        // Capture the feed into the history entry on the way out: pointerdown
        // fires while still on `/tracks`, before the row `<Link>` navigates.
        onPointerDown={feed.persist}
      >
        <thead>
          <tr>
            <th className={`${styles.colIdx} ${styles.alignRight}`}>#</th>
            <th className={styles.colTakeoff}>Takeoff</th>
            <th>Pilot</th>
            <th className={`${styles.colDuration} ${styles.alignRight}`}>
              Duration
            </th>
            <th className={styles.colScore}>Score</th>
            <th className={styles.colDist}>Distance</th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ item, cells, date }, index) => {
            const showDate = index === 0 || rows[index - 1].date !== date;
            return (
              <Fragment key={item.track.id}>
                {showDate && (
                  <tr className={styles.dateRow}>
                    <td colSpan={COLUMN_COUNT}>{date}</td>
                  </tr>
                )}
                <TrackRow item={item} cells={cells} />
              </Fragment>
            );
          })}
          {feed.isLoading && <SkeletonRows colSpan={COLUMN_COUNT} />}
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
      key: 'takeoff',
      content: item.track.takeoff.name ? (
        <>
          {item.track.takeoff.country && (
            <>
              <Flag code={item.track.takeoff.country} />
              &nbsp;&nbsp;
            </>
          )}
          {item.track.takeoff.name}
        </>
      ) : (
        '—'
      ),
      muted: item.track.takeoff.name == null,
      className: styles.colTakeoff,
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
const COLUMN_COUNT = 6;
