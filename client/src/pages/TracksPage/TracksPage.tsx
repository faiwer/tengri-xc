import { Skeleton } from 'antd';
import { Fragment, useMemo } from 'react';
import type { RouteType, TrackListItem } from '../../api/tracks.io';
import { TextWithIcon } from '../../components/TextWithIcon';
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
import { useMediaQuery } from '../../utils/useMediaQuery';
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

  const layout = useLayout();
  const { gridTemplateColumns, thead } = useColumns(layout);

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
        cells: buildHomeRowCells(item, index + 1, prefs, layout),
        date: formatShortDate(
          item.track.takeoffAt,
          prefs,
          item.track.takeoffOffset,
        ),
      })),
    [feed.items, prefs, layout],
  );

  const isEmpty = feed.items?.length === 0 && !feed.isLoading;
  const hasInlineError = feed.error !== null && feed.items === null;
  useErrorToast(feed.error, { title: "Couldn't load flights" });

  return (
    <PageLayout>
      <table
        className={styles.table}
        style={{ gridTemplateColumns }}
        // Capture the feed into the history entry on the way out: pointerdown
        // fires while still on `/tracks`, before the row `<Link>` navigates.
        onPointerDown={feed.persist}
      >
        {thead}
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

function useLayout() {
  const layout = useMediaQuery({
    micro: ['<', 450],
    tiny: ['<', 550],
    small: ['<', 650],
    medium: ['<', 750],
    normal: ['>=', 750],
  });

  return {
    layout,
    showTakeoff: layout !== 'micro',
    showDuration: layout !== 'tiny',
    showIdx: layout === 'normal',
    showScore: layout === 'normal' || layout === 'medium',
  };
}

function useColumns(layout: ReturnType<typeof useLayout>) {
  const gridTemplateColumns = gridColumns([
    [layout.showIdx, 'max-content'], // #
    [layout.showTakeoff, '1fr'], // takeoff
    [true, '1fr'], // pilot
    [layout.showDuration, 'max-content'], // duration
    [layout.showScore, 'max-content'], // score
    [true, 'max-content'], // distance
  ]);

  const thead = (
    <thead>
      <tr>
        {layout.showIdx && <th className={styles.alignRight}>#</th>}
        {layout.showTakeoff && <th>Takeoff</th>}
        <th>Pilot</th>
        {layout.showDuration && (
          <th className={styles.alignRight}>
            {layout.layout === 'micro' ? '' : 'Duration'}
          </th>
        )}
        {layout.showScore && <th>Score</th>}
        <th>Distance</th>
      </tr>
    </thead>
  );

  return { gridTemplateColumns, thead };
}

function buildHomeRowCells(
  item: TrackListItem,
  rowNumber: number,
  prefs: ResolvedPreferences,
  {
    showScore,
    showIdx,
    showDuration,
    showTakeoff,
  }: ReturnType<typeof useLayout>,
): TrackRowCell[] {
  const cells: Array<TrackRowCell | false> = [
    showIdx && {
      key: 'idx',
      content: rowNumber,
      align: 'right',
    },
    showTakeoff && {
      key: 'takeoff',
      content: item.track.takeoff.name ? (
        <TextWithIcon
          flag={item.track.takeoff.country}
          text={item.track.takeoff.name}
        />
      ) : (
        '—'
      ),
      muted: item.track.takeoff.name == null,
    },
    {
      key: 'pilot',
      content: (
        <TextWithIcon flag={item.pilot.country} text={item.pilot.name} />
      ),
    },
    showDuration && {
      key: 'duration',
      content: formatDuration(item.track.duration),
      align: 'right',
    },
    showScore && {
      key: 'score',
      content: formatScore(item.track.mainRouteType, item.track.mainScore),
      align: 'left',
      muted: item.track.mainScore == null,
    },
    {
      key: 'dist',
      content: formatDistanceScored(item, showScore, prefs),
      align: 'left',
      muted: item.track.mainDistance == null,
    },
  ];
  return cells.filter((cell) => !!cell);
}

/** Space-joined grid tracks for the columns that are currently visible. */
const gridColumns = (
  columns: Array<[visible: boolean, track: string]>,
): string =>
  columns
    .filter(([visible]) => visible)
    .map(([, track]) => track)
    .join(' ');

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

const formatDistanceScored = (
  item: TrackListItem,
  showScore: boolean,
  prefs: ResolvedPreferences,
): React.ReactNode => {
  const { mainRouteType, mainDistance } = item.track;
  return (
    <>
      {showScore || !mainRouteType ? null : (
        <>
          <RouteTypeIcon kind={mainRouteType} />
          &nbsp;&nbsp;
        </>
      )}
      {mainDistance ? formatDistance(mainDistance, prefs) : '—'}
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
