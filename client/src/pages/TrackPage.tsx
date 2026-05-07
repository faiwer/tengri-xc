import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router';
import { getTrack, getTrackMetadata } from '../api/tracks';
import type { TrackMetadata } from '../api/tracks.io';
import { MapView } from '../components/MapView';

type LoadState =
  | { status: 'loading' }
  | { status: 'ok'; data: TrackMetadata }
  | { status: 'error'; message: string };

export function TrackPage() {
  const { id } = useParams<{ id: string }>();
  const [state, setState] = useState<LoadState>({ status: 'loading' });

  useEffect(() => {
    if (!id) return;
    const ctrl = new AbortController();
    setState({ status: 'loading' });

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
      .then((track) => {
        console.log('track', track);
        const n = track.t.length;
        const kind = track.baroAlt ? 'dual' : 'gps';
        console.log(
          `track summary: kind=${kind} length=${n} start_time=${track.startTime} t[0]=${track.t[0]} t[last]=${track.t[n - 1]}`,
        );
      })
      .catch((err: unknown) => {
        if (err instanceof DOMException && err.name === 'AbortError') return;
        console.error('track decode failed', err);
      });

    return () => {
      ctrl.abort();
    };
  }, [id]);

  return (
    <div>
      <h1>Track: {id}</h1>
      {state.status === 'loading' && <p>Loading…</p>}
      {state.status === 'ok' && <p>Pilot: {state.data.pilot.name}</p>}
      {state.status === 'error' && <p>Error: {state.message}</p>}
      <MapView />
      <Link to="/">Back</Link>
    </div>
  );
}
