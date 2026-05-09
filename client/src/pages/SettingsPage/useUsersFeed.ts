import { useEffect, useState } from 'react';

import { getUsersPage } from '../../api/admin/users';
import type { UserListItem } from '../../api/admin/users.io';
import {
  useAsyncEffect,
  useDebouncedValue,
  useEventHandler,
} from '../../core/hooks';

interface FeedState {
  items: UserListItem[] | null;
  cursor: string | null;
  nextCursor: string | null;
  isLoading: boolean;
  error: string | null;
}

export interface UsersFeed {
  items: UserListItem[] | null;
  isLoading: boolean;
  /** No `nextCursor` *and* at least one page came back. */
  completed: boolean;
  error: string | null;
  query: string;
  setQuery: (q: string) => void;
  loadMore: () => void;
}

/** Wait this long after the last keystroke before refetching. */
const SEARCH_DEBOUNCE_MS = 250;

/**
 * Owns the users feed: debounced search, cursor-paginated fetching,
 * item accumulation, loading/error state. Search resets the cursor
 * and clears items so the user sees the first matching page
 * immediately. Pagination only goes forward (cursors are opaque); the
 * UI exposes a "Load more" affordance, not page numbers.
 */
export function useUsersFeed(): UsersFeed {
  const [query, setQuery] = useState('');
  const debouncedQuery = useDebouncedValue(query, SEARCH_DEBOUNCE_MS);
  const [state, setState] = useState<FeedState>(INITIAL_STATE);

  // Reset pagination whenever the search settles. Fires in the same
  // commit as the fetch effect below; the first fetch (with stale
  // `state.cursor`) gets aborted by the next effect cycle, the second
  // one runs against `cursor: null`. One wasted abort per query
  // change, no stale-page bleed-through.
  useEffect(() => {
    setState({ ...INITIAL_STATE });
  }, [debouncedQuery]);

  const loadMore = useEventHandler(() => {
    if (state.isLoading || state.nextCursor === null) {
      return;
    }
    setState((s) => ({ ...s, cursor: s.nextCursor, isLoading: true }));
  });

  useAsyncEffect(
    async (signal) => {
      try {
        const page = await getUsersPage({
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
    [debouncedQuery, state.cursor],
  );

  return {
    items: state.items,
    isLoading: state.isLoading,
    error: state.error,
    completed: state.items !== null && state.nextCursor === null,
    query,
    setQuery,
    loadMore,
  };
}

const INITIAL_STATE: FeedState = {
  items: null,
  cursor: null,
  nextCursor: null,
  isLoading: true,
  error: null,
};
