import { useEffect, useMemo, useState } from 'react';
import { Link, useParams } from 'react-router';
import { getTrack, getTrackMetadata } from '../api/tracks';
import type { TrackMetadata } from '../api/tracks.io';
import { FitBounds, MapView, TrackPolyline } from '../components/MapView';
import { TrackMetaPanel } from '../components/TrackMetaPanel';
import { pathsBounds, trackToPaths } from '../track/toPaths';
import type { Track } from '../track';
import styles from './TrackPage.module.scss';

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

  const paths = useMemo(() => (track ? trackToPaths(track) : null), [track]);
  const bounds = useMemo(() => (paths ? pathsBounds(paths) : null), [paths]);

  return (
    <div className={styles.page}>
      {state.status === 'loading' && <p>Loading…</p>}
      {state.status === 'ok' && <TrackMetaPanel data={state.data} />}
      {state.status === 'error' && <p>Error: {state.message}</p>}
      <MapView>
        {paths && <TrackPolyline paths={paths} />}
        <FitBounds bounds={bounds} />
      </MapView>
      <Link to="/">Back</Link>
    </div>
  );
}
