import { Alert, Button, Skeleton } from 'antd';
import { useMemo } from 'react';
import type { TrackListItem } from '../../api/tracks.io';
import { Flag } from '../../components/Flag';
import { PageLayout } from '../../components/PageLayout';
import { TrackRow, type TrackRowCell } from '../../components/TrackRow';
import { useErrorToast } from '../../core/hooks';
import {
  usePreferences,
  type ResolvedPreferences,
} from '../../core/preferences';
import {
  formatDuration,
  formatShortDate,
  formatShortTime,
} from '../../utils/formatDateTime';
import styles from './TracksPage.module.scss';
import { useScrollSentinel } from './useScrollSentinel';
import { useTracksFeed } from './useTracksFeed';

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
            <th className={styles.colTime}>Takeoff</th>
            <th>Pilot</th>
            <th className={`${styles.colDuration} ${styles.alignRight}`}>
              Duration
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ item, cells }) => (
            <TrackRow key={item.track.id} item={item} cells={cells} />
          ))}
          {feed.isLoading && <SkeletonRows />}
        </tbody>
      </table>

      {isEmpty && <p className={styles.empty}>No flights yet.</p>}

      {hasInlineError && (
        <Alert
          type="error"
          showIcon
          title="Couldn't load flights"
          description={feed.error}
          action={
            <Button size="small" onClick={() => window.location.reload()}>
              Reload
            </Button>
          }
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
      key: 'time',
      content: formatShortTime(
        item.track.takeoffAt,
        prefs,
        item.track.takeoffOffset,
      ),
      className: styles.colTime,
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
  ];
}

function SkeletonRows() {
  return (
    <>
      {Array.from({ length: LOADING_SKELETON_COUNT }, (_, i) => (
        <tr key={`sk-${i}`} className={styles.skeletonRow}>
          <td colSpan={5}>
            <Skeleton.Input active block size="small" />
          </td>
        </tr>
      ))}
    </>
  );
}

const LOADING_SKELETON_COUNT = 8;
