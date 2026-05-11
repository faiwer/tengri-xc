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
  /** Re-run the most recent fetch in place. */
  retry: () => void;
}

/**
 * Owns the tracks feed: cursor-paginated fetching, item accumulation,
 * loading/error state.
 */
export function useTracksFeed(): TracksFeed {
  const [state, setState] = useState<FeedState>(INITIAL_STATE);
  // Bumped by `retry()` to re-fire the fetch effect without changing the
  // cursor — a real new attempt rather than just clearing the error
  // banner.
  const [retryToken, setRetryToken] = useState(0);

  const loadMore = useEventHandler(() => {
    if (state.isLoading) {
      return;
    }

    if (state.nextCursor === null && state.items !== null) {
      return;
    }

    setState((s) => ({ ...s, cursor: s.nextCursor }));
  });

  const retry = useEventHandler(() => setRetryToken((t) => t + 1));

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
    [state.cursor, retryToken],
  );

  return {
    items: state.items,
    isLoading: state.isLoading,
    error: state.error,
    loadMore,
    retry,
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
