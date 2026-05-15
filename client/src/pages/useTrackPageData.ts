import { useState } from 'react';
import { getTrack, getTrackMetadata } from '../api/tracks';
import type { TrackMetadata } from '../api/tracks.io';
import { useAsyncEffect } from '../core/hooks';
import type { Track } from '../track';

type LoadState<T> =
  | { status: 'loading' }
  | { status: 'ok'; data: T }
  | { status: 'error'; error: unknown };

type MetadataState = LoadState<TrackMetadata>;
type TrackState = LoadState<Track>;

export function useTrackPageData(id: string) {
  const [state, setState] = useState<MetadataState>({ status: 'loading' });
  const [trackState, setTrackState] = useState<TrackState>({
    status: 'loading',
  });

  useAsyncEffect(
    async (signal) => {
      setState({ status: 'loading' });
      setTrackState({ status: 'loading' });

      await Promise.all([
        loadMetadata(id, signal, setState),
        loadTrack(id, signal, setTrackState),
      ]);
    },
    [id],
  );

  return {
    state,
    trackState,
    track: trackState.status === 'ok' ? trackState.data : null,
  };
}

const loadMetadata = async (
  id: string,
  signal: AbortSignal,
  setState: (state: MetadataState) => void,
): Promise<void> => {
  try {
    const data = await getTrackMetadata(id);
    if (!signal.aborted) {
      setState({ status: 'ok', data });
    }
  } catch (err: unknown) {
    if (!signal.aborted) {
      setState({ status: 'error', error: err });
    }
  }
};

const loadTrack = async (
  id: string,
  signal: AbortSignal,
  setTrackState: (state: TrackState) => void,
): Promise<void> => {
  try {
    const decoded = await getTrack(id, 'full', { signal });
    if (!signal.aborted) {
      setTrackState({ status: 'ok', data: decoded });
    }
  } catch (err: unknown) {
    if (!signal.aborted) {
      setTrackState({ status: 'error', error: err });
    }
  }
};
