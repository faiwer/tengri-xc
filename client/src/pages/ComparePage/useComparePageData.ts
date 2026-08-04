import { useState } from 'react';
import { getTrack, getTrackMetadata } from '../../api/tracks';
import type { TrackMetadata } from '../../api/tracks.io';
import { useAsyncEffect } from '../../core/hooks';
import type { Track } from '../../track';
import { colorForIndex } from './colors';

type LoadState<T> =
  | { status: 'loading' }
  | { status: 'ok'; data: T }
  | { status: 'error'; error: unknown };

export interface CompareFlight {
  id: string;
  /** Palette colour for this flight's track and sidebar swatch. */
  color: string;
  metadata: LoadState<TrackMetadata>;
  track: LoadState<Track>;
}

export function useComparePageData(ids: string[]): CompareFlight[] {
  const [flights, setFlights] = useState<CompareFlight[]>(() => initial(ids));

  useAsyncEffect(
    (signal) => {
      setFlights(initial(ids));

      ids.forEach((id, index) => {
        void loadMetadata(id, index, signal, setFlights);
        void loadTrack(id, index, signal, setFlights);
      });
    },
    [ids.join(',')],
  );

  return flights;
}

type SetFlights = (updater: (prev: CompareFlight[]) => CompareFlight[]) => void;

const loadMetadata = async (
  id: string,
  index: number,
  signal: AbortSignal,
  setFlights: SetFlights,
): Promise<void> => {
  try {
    const data = await getTrackMetadata(id);
    if (!signal.aborted) {
      setFlights((prev) =>
        patch(prev, index, id, { metadata: { status: 'ok', data } }),
      );
    }
  } catch (error: unknown) {
    if (!signal.aborted) {
      setFlights((prev) =>
        patch(prev, index, id, { metadata: { status: 'error', error } }),
      );
    }
  }
};

const loadTrack = async (
  id: string,
  index: number,
  signal: AbortSignal,
  setFlights: SetFlights,
): Promise<void> => {
  try {
    const decoded = await getTrack(id, 'full', { signal });
    if (!signal.aborted) {
      setFlights((prev) =>
        patch(prev, index, id, { track: { status: 'ok', data: decoded } }),
      );
    }
  } catch (error: unknown) {
    if (!signal.aborted) {
      setFlights((prev) =>
        patch(prev, index, id, { track: { status: 'error', error } }),
      );
    }
  }
};

const initial = (ids: string[]): CompareFlight[] =>
  ids.map((id, index) => ({
    id,
    color: colorForIndex(index),
    metadata: { status: 'loading' },
    track: { status: 'loading' },
  }));

/** Immutably update one flight, guarding against index/id drift across reloads. */
const patch = (
  prev: CompareFlight[],
  index: number,
  id: string,
  changes: Partial<CompareFlight>,
): CompareFlight[] =>
  prev.map((flight, i) =>
    i === index && flight.id === id ? { ...flight, ...changes } : flight,
  );
