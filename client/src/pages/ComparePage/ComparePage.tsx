import { useMemo, useRef } from 'react';
import { useParams } from 'react-router';
import { FitBounds, MapView, TrackPolyline } from '../../components/MapView';
import { PageLayout } from '../../components/PageLayout';
import { pathsBounds, trackToPaths, type TrackPath } from '../../track/toPaths';
import { CompareList } from './CompareList';
import { useComparePageData } from './useComparePageData';
import styles from './ComparePage.module.scss';

export function ComparePage() {
  const { ids: idsParam } = useParams() as { ids: string };
  const ids = useMemo(() => idsParam.split(','), [idsParam]);
  const rightRef = useRef<HTMLDivElement>(null);
  const flights = useComparePageData(ids);

  const paths = useMemo<TrackPath[]>(() => {
    const result: TrackPath[] = [];
    for (const flight of flights) {
      if (flight.track.status === 'ok') {
        for (const path of trackToPaths(flight.track.data)) {
          result.push({ ...path, color: flight.color });
        }
      }
    }
    return result;
  }, [flights]);

  const bounds = useMemo(() => pathsBounds(paths), [paths]);

  return (
    <PageLayout>
      <div className={styles.layout}>
        <aside className={styles.left} tengri-theme="dark">
          <CompareList flights={flights} />
        </aside>
        <div ref={rightRef} className={styles.right}>
          <div className={styles.mapSlot}>
            {bounds && (
              <MapView initialBounds={bounds} fullscreenContainerRef={rightRef}>
                <TrackPolyline paths={paths} />
                <FitBounds bounds={bounds} skipInitialFit={!!bounds} />
              </MapView>
            )}
          </div>
        </div>
      </div>
    </PageLayout>
  );
}
