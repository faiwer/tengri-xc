import { useEffect, useState } from 'react';

import { getSitesPage } from '../../api/admin/sites';
import type { SiteListItem } from '../../api/admin/sites.io';
import {
  useAsyncEffect,
  useDebouncedValue,
  useEventHandler,
} from '../../core/hooks';

interface FeedState {
  items: SiteListItem[] | null;
  cursor: string | null;
  nextCursor: string | null;
  isLoading: boolean;
  error: string | null;
}

export interface SitesFeed {
  items: SiteListItem[] | null;
  isLoading: boolean;
  /** No `nextCursor` *and* at least one page came back. */
  completed: boolean;
  error: string | null;
  query: string;
  setQuery: (q: string) => void;
  loadMore: () => void;
  /** Re-run the most recent fetch in place. */
  retry: () => void;
}

/** Wait this long after the last keystroke before refetching. */
const SEARCH_DEBOUNCE_MS = 250;

/**
 * Owns the sites feed: debounced name search, cursor-paginated fetching, item
 * accumulation, loading/error state. Search resets the cursor and clears items
 * so the user sees the first matching page immediately. Pagination only goes
 * forward (cursors are opaque); the UI exposes a "Load more" affordance, not
 * page numbers.
 */
export function useSitesFeed(): SitesFeed {
  const [query, setQuery] = useState('');
  const debouncedQuery = useDebouncedValue(query, SEARCH_DEBOUNCE_MS);
  const [state, setState] = useState<FeedState>(INITIAL_STATE);
  // Bumped by `retry()` to re-fire the fetch effect with the same cursor + query.
  const [retryToken, setRetryToken] = useState(0);

  // Reset pagination whenever the search settles.
  useEffect(() => {
    setState({ ...INITIAL_STATE });
  }, [debouncedQuery]);

  const loadMore = useEventHandler(() => {
    if (!state.isLoading && state.nextCursor !== null) {
      setState((s) => ({ ...s, cursor: s.nextCursor, isLoading: true }));
    }
  });

  const retry = useEventHandler(() => setRetryToken((t) => t + 1));

  useAsyncEffect(
    async (signal) => {
      try {
        const page = await getSitesPage({
          q: debouncedQuery || undefined,
          cursor: state.cursor ?? undefined,
          signal,
        });
        if (!signal.aborted) {
          setState((s) => ({
            // First page of a fresh search replaces; subsequent pages
            // append. `cursor === null` is the "fresh search" marker.
            items:
              s.cursor === null
                ? page.items
                : [...(s.items ?? []), ...page.items],
            cursor: s.cursor,
            nextCursor: page.nextCursor,
            isLoading: false,
            error: null,
          }));
        }
      } catch (err: unknown) {
        if (!signal.aborted) {
          setState((s) => ({
            ...s,
            isLoading: false,
            error: err instanceof Error ? err.message : String(err),
          }));
        }
      }
    },
    [debouncedQuery, state.cursor, retryToken],
  );

  return {
    items: state.items,
    isLoading: state.isLoading,
    error: state.error,
    completed: state.items !== null && state.nextCursor === null,
    query,
    setQuery,
    loadMore,
    retry,
  };
}

const INITIAL_STATE: FeedState = {
  items: null,
  cursor: null,
  nextCursor: null,
  isLoading: true,
  error: null,
};
