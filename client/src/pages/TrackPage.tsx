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
    let cancelled = false;
    setState({ status: 'loading' });

    getTrackMetadata(id)
      .then((data) => {
        if (!cancelled) setState({ status: 'ok', data });
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          const message = err instanceof Error ? err.message : String(err);
          setState({ status: 'error', message });
        }
      });

    getTrack(id)
      .then((track) => {
        if (!cancelled) {
          console.log('track', track);
          console.log(
            `track summary: version=${track.version} start_time=${track.track.start_time} interval=${track.track.interval} time_fixes=${track.track.time_fixes.length}`,
          );
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) console.error('track decode failed', err);
      });

    return () => {
      cancelled = true;
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
