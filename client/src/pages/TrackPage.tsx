import { useEffect, useMemo, useState } from 'react';
import { useParams } from 'react-router';
import { getTrack, getTrackMetadata } from '../api/tracks';
import type { TrackMetadata } from '../api/tracks.io';
import {
  FitBounds,
  MapView,
  TrackHoverMarker,
  TrackPolyline,
} from '../components/MapView';
import { FlightChart } from '../components/FlightChart';
import { PageLayout } from '../components/PageLayout';
import { TrackMetaPanel } from '../components/TrackMetaPanel';
import { altitudeRange } from '../track/altitudeRange';
import { findIndexAt } from '../track/findIndexAt';
import { trackToPaths, type TrackWindow } from '../track/toPaths';
import { computeVarioInsights, type VarioPeaks } from '../track/varioSegments';
import type { Track } from '../track';
import styles from './TrackPage.module.scss';
import { useTrackBounds } from './useTrackBounds';
import { useTrackHoverPoint } from './useTrackHoverPoint';

type LoadState =
  | { status: 'loading' }
  | { status: 'ok'; data: TrackMetadata }
  | { status: 'error'; message: string };

export function TrackPage() {
  const { id } = useParams<{ id: string }>();
  const [state, setState] = useState<LoadState>({ status: 'loading' });
  const [track, setTrack] = useState<Track | null>(null);

  useEffect(() => {
    if (!id) return;
    const ctrl = new AbortController();
    setState({ status: 'loading' });
    setTrack(null);

    getTrackMetadata(id)
      .then((data) => {
        if (!ctrl.signal.aborted) {
          setState({ status: 'ok', data });
        }
      })
      .catch((err: unknown) => {
        if (ctrl.signal.aborted) return;
        const message = err instanceof Error ? err.message : String(err);
        setState({ status: 'error', message });
      });

    getTrack(id, 'full', { signal: ctrl.signal })
      .then((decoded) => {
        if (!ctrl.signal.aborted) setTrack(decoded);
      })
      .catch((err: unknown) => {
        if (err instanceof DOMException && err.name === 'AbortError') return;
        console.error('track decode failed', err);
      });

    return () => {
      ctrl.abort();
    };
  }, [id]);

  const window = useMemo<TrackWindow | undefined>(() => {
    if (!track || state.status !== 'ok') return undefined;
    return {
      takeoffIdx: findIndexAt(track, state.data.takeoffAt),
      landingIdx: findIndexAt(track, state.data.landingAt),
    };
  }, [track, state]);

  const insights = useMemo(() => {
    if (!track || !window) return undefined;
    return computeVarioInsights(
      track,
      window.takeoffIdx,
      window.landingIdx + 1,
    );
  }, [track, window]);

  const peaks: VarioPeaks | undefined = insights
    ? { peakClimb: insights.peakClimb, peakSink: insights.peakSink }
    : undefined;

  const altitudes = useMemo(() => {
    if (!track || !window) return undefined;
    return altitudeRange(track, window.takeoffIdx, window.landingIdx + 1);
  }, [track, window]);

  const paths = useMemo(
    () => (track ? trackToPaths(track, window, insights?.segments) : null),
    [track, window, insights],
  );
  const bounds = useTrackBounds(track, window);
  const {
    point: hoverPoint,
    mapHoverFraction,
    setHoverFraction,
    setHoverLatLng,
  } = useTrackHoverPoint(track, window, bounds);

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
              peaks={peaks}
              altitudes={altitudes}
            />
          )}
          {state.status === 'error' && (
            <p className={styles.statusMessage}>Error: {state.message}</p>
          )}
        </aside>
        <div className={styles.right}>
          <div className={styles.mapSlot}>
            <MapView onHoverLatLng={setHoverLatLng}>
              {paths && <TrackPolyline paths={paths} />}
              <TrackHoverMarker point={hoverPoint} />
              <FitBounds bounds={bounds} />
            </MapView>
          </div>
          {track && window && (
            <FlightChart
              track={track}
              window={window}
              onHoverFractionChange={setHoverFraction}
              hoverFraction={mapHoverFraction}
            />
          )}
        </div>
      </div>
    </PageLayout>
  );
}
