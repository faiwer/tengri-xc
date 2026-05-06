import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router';
import { getTrackMetadata } from '../api/tracks';
import type { TrackMetadata } from '../api/tracks.io';

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
      <Link to="/">Back</Link>
    </div>
  );
}
