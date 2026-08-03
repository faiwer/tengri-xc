import { useMemo, useRef, useState } from 'react';
import { useParams } from 'react-router';
import {
  FitBounds,
  MapView,
  TrackHoverMarker,
  TrackPolyline,
} from '../components/MapView';
import { TrackRoute } from '../components/TrackRoute';
import type { LatLng } from '../utils/geo/coordinates';
import { FlightChart, useChartKind } from '../components/FlightChart';
import { PageLayout } from '../components/PageLayout';
import { TrackMetaPanel } from '../components/TrackMetaPanel';
import { debounce } from '../utils/debounce';
import { CursorReadout } from './CursorReadout/index';
import styles from './TrackPage.module.scss';
import { useFlightAnalysis } from './useFlightAnalysis';
import { useGroundElevations } from './useGroundElevations';
import { useRoute } from './useRoute';
import { useTrackHoverPoint } from './useTrackHoverPoint';
import { useTrackPageData } from './useTrackPageData';
import { LoadingIcon } from '../components/icons/LoadingIcon';
import { ErrorPane } from '../components/ErrorPane/ErrorPane';
import { MobileTrackInfo } from './MobileTrackInfo';

export function TrackPage() {
  const { id } = useParams() as { id: string };
  const rightRef = useRef<HTMLDivElement>(null);
  const [mapCenter, setMapCenter] = useState<LatLng | null>(null);
  const [activeChartKind, setActiveChartKind] = useChartKind();
  const setMapCenterDebounced = useMemo(() => debounce(setMapCenter, 500), []);
  const { state, trackState, track } = useTrackPageData(id);
  const metadata = state.status === 'ok' ? state.data : null;
  const { selectedRoute, onRouteSelect } = useRoute(metadata);
  const analysis = useFlightAnalysis(
    track,
    state.status === 'ok' ? state.data : undefined,
  );
  const { ground } = useGroundElevations(track, analysis?.window);
  const chartLoading =
    (state.status === 'loading' || trackState.status === 'loading') &&
    state.status !== 'error' &&
    trackState.status !== 'error';
  const {
    point: hoverPoint,
    trackIndex: hoverTrackIndex,
    chartHoverFraction,
    clearHover,
    setHoverFraction,
    setHoverLatLng,
  } = useTrackHoverPoint(track, analysis?.window, analysis?.bounds);
  const [mobileLayout, setMobileLayout] = useState<'info' | 'chart'>('info');

  return (
    <PageLayout>
      <div className={styles.layout}>
        <aside className={styles.left} tengri-theme="dark">
          {state.status === 'loading' && <Loading inverseTheme />}
          {state.status === 'ok' && (
            <TrackMetaPanel
              data={state.data}
              selectedRoute={selectedRoute}
              onRouteSelect={onRouteSelect}
              hasAltitudeData={analysis?.hasAltitudeData}
              peaks={analysis?.vario}
              altitudes={analysis?.altitudes}
            />
          )}
          {state.status === 'error' && (
            <ErrorPane
              title="Could not download flight metadata"
              error={state.error}
              valign="top"
            />
          )}
        </aside>
        <div
          ref={rightRef}
          className={styles.right}
          onPointerLeave={clearHover}
        >
          <div className={styles.mapSlot}>
            {trackState.status === 'error' ? (
              <ErrorPane
                title="Couldn't load flight track"
                error={trackState.error}
              />
            ) : trackState.status === 'loading' ? (
              <Loading />
            ) : (
              <MapView
                initialBounds={analysis?.bounds ?? null}
                onCenterLatLng={setMapCenterDebounced}
                onHoverLatLng={setHoverLatLng}
                fullscreenContainerRef={rightRef}
              >
                {analysis && <TrackPolyline paths={analysis.paths} />}
                {selectedRoute && <TrackRoute route={selectedRoute} />}
                <TrackHoverMarker point={hoverPoint} />
                <FitBounds
                  bounds={analysis?.bounds ?? null}
                  skipInitialFit={!!analysis?.bounds}
                />
              </MapView>
            )}
          </div>
          <CursorReadout
            activeChartKind={activeChartKind}
            analysis={analysis}
            ground={ground}
            mapCenter={mapCenter}
            trackIndex={hoverTrackIndex}
            mobileLayout={mobileLayout}
            onMobileLayoutChange={setMobileLayout}
          />
          <div className={styles.bottom} data-mobile-layout={mobileLayout}>
            {track && analysis && metadata ? (
              <>
                <FlightChart
                  className={styles.chart}
                  activeKind={activeChartKind}
                  track={track}
                  analysis={analysis}
                  ground={ground}
                  onActiveKindChange={setActiveChartKind}
                  onHoverFractionChange={setHoverFraction}
                  hoverFraction={chartHoverFraction}
                />
                <MobileTrackInfo
                  className={styles.info}
                  metadata={metadata}
                  analysis={analysis}
                  selectedRoute={selectedRoute}
                />
              </>
            ) : chartLoading ? (
              <div className={styles.chartLoadingSlot}>
                <Loading />
              </div>
            ) : null}
          </div>
        </div>
      </div>
    </PageLayout>
  );
}

const Loading = ({ inverseTheme = false }: { inverseTheme?: boolean }) => (
  <div className={styles.loading}>
    <LoadingIcon inverseTheme={inverseTheme} />
  </div>
);
