import { useMemo, useState } from 'react';
import { useParams } from 'react-router';
import {
  FitBounds,
  MapView,
  TrackHoverMarker,
  TrackPolyline,
} from '../components/MapView';
import { FlightChart } from '../components/FlightChart';
import { PageLayout } from '../components/PageLayout';
import { TrackMetaPanel } from '../components/TrackMetaPanel';
import { debounce } from '../utils/debounce';
import { CursorReadout } from './CursorReadout/index';
import styles from './TrackPage.module.scss';
import { useFlightAnalysis } from './useFlightAnalysis';
import { useTrackHoverPoint } from './useTrackHoverPoint';
import { useTrackPageData } from './useTrackPageData';

export function TrackPage() {
  const { id } = useParams() as { id: string };
  const [mapCenter, setMapCenter] = useState<google.maps.LatLngLiteral | null>(
    null,
  );
  const setMapCenterDebounced = useMemo(() => debounce(setMapCenter, 500), []);
  const { state, trackState, track } = useTrackPageData(id);
  const analysis = useFlightAnalysis(
    track,
    state.status === 'ok' ? state.data : undefined,
  );
  const {
    point: hoverPoint,
    trackIndex: hoverTrackIndex,
    chartHoverFraction,
    clearHover,
    setHoverFraction,
    setHoverLatLng,
  } = useTrackHoverPoint(track, analysis?.window, analysis?.bounds);

  return (
    <PageLayout>
      <div className={styles.layout}>
        <aside className={styles.left}>
          {state.status === 'loading' && (
            <p className={styles.statusMessage}>Loading…</p>
          )}
          {state.status === 'ok' && (
            <TrackMetaPanel
              data={state.data}
              peaks={analysis?.vario}
              altitudes={analysis?.altitudes}
            />
          )}
          {state.status === 'error' && (
            <ErrorMessage
              title="Could not download flight metadata"
              error={state.error}
              className={styles.statusMessage}
            />
          )}
        </aside>
        <div className={styles.right} onPointerLeave={clearHover}>
          <div className={styles.mapSlot}>
            {trackState.status === 'error' ? (
              <ErrorMessage
                title="Couldn't load flight track"
                error={trackState.error}
                className={styles.mapError}
              />
            ) : (
              <MapView
                onCenterLatLng={setMapCenterDebounced}
                onHoverLatLng={setHoverLatLng}
              >
                {analysis && <TrackPolyline paths={analysis.paths} />}
                <TrackHoverMarker point={hoverPoint} />
                <FitBounds bounds={analysis?.bounds ?? null} />
              </MapView>
            )}
          </div>
          <CursorReadout
            analysis={analysis}
            mapCenter={mapCenter}
            trackIndex={hoverTrackIndex}
          />
          {track && analysis && (
            <FlightChart
              track={track}
              analysis={analysis}
              onHoverFractionChange={setHoverFraction}
              hoverFraction={chartHoverFraction}
            />
          )}
        </div>
      </div>
    </PageLayout>
  );
}

const ErrorMessage = ({
  error,
  className,
  title,
}: {
  error: unknown;
  className: string;
  title: string;
}) => {
  const msg = error instanceof Error ? error.message : String(error);
  return (
    <div className={className}>
      {title}:
      <br />
      {msg}
    </div>
  );
};
