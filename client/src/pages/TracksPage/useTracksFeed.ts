import { useState } from 'react';
import { getTracksPage } from '../../api/tracks';
import type { TrackListItem } from '../../api/tracks.io';
import { useAsyncEffect, useEventHandler } from '../../core/hooks';

interface FeedState {
  items: TrackListItem[] | null;
  cursor: string | null;
  nextCursor: string | null;
  isLoading: boolean;
  error: string | null;
}

export interface TracksFeed {
  items: TrackListItem[] | null;
  isLoading: boolean;
  completed: boolean;
  error: string | null;
  loadMore: () => void;
}

/**
 * Owns the tracks feed: cursor-paginated fetching, item accumulation,
 * loading/error state.
 */
export function useTracksFeed(): TracksFeed {
  const [state, setState] = useState<FeedState>(INITIAL_STATE);

  const loadMore = useEventHandler(() => {
    if (state.isLoading) {
      return;
    }

    if (state.nextCursor === null && state.items !== null) {
      return;
    }

    setState((s) => ({ ...s, cursor: s.nextCursor }));
  });

  useAsyncEffect(
    async (signal) => {
      setState((s) => ({ ...s, isLoading: true, error: null }));

      try {
        const page = await getTracksPage({
          cursor: state.cursor ?? undefined,
          signal,
        });
        if (!signal.aborted) {
          setState((s) => ({
            ...s,
            items: [...(s.items ?? []), ...page.items],
            nextCursor: page.nextCursor,
            isLoading: false,
            error: null,
          }));
        }
      } catch (err: unknown) {
        if (!signal.aborted) {
          const message = err instanceof Error ? err.message : String(err);
          setState((s) => ({ ...s, isLoading: false, error: message }));
        }
      }
    },
    [state.cursor],
  );

  return {
    items: state.items,
    isLoading: state.isLoading,
    error: state.error,
    loadMore,
    completed: state.items !== null && state.nextCursor === null,
  };
}

const INITIAL_STATE: FeedState = {
  items: null,
  cursor: null,
  nextCursor: null,
  isLoading: true,
  error: null,
};
