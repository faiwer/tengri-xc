import { useState } from 'react';

import { getGliderCatalog } from '../../../api/admin/gliders';
import type { GliderCatalog, Sport } from '../../../api/admin/gliders.io';
import {
  useAsyncEffect,
  useErrorToast,
  useEventHandler,
} from '../../../core/hooks';

/**
 * Fetches the glider catalog for `sport`, refetching when the sport changes or
 * `reload()` is called. Stale data is cleared on each `sport` switch so the
 * caller doesn't show the previous sport's tree while the new one loads. Errors
 * surface via toast and via the returned `error` field — the caller picks what
 * to render.
 */
export function useGliderCatalog(sport: Sport) {
  const [state, setState] = useState<State>(INITIAL_STATE);
  // Bumped by `reload()` to re-fire the fetch effect with the same sport.
  const [reloadToken, setReloadToken] = useState(0);

  useErrorToast(state.error, { title: "Couldn't load gliders" });

  useAsyncEffect(
    async (signal) => {
      setState({ data: null, isLoading: true, error: null });
      try {
        const catalog = await getGliderCatalog(sport, { signal });
        if (!signal.aborted) {
          setState({ data: catalog, isLoading: false, error: null });
        }
      } catch (err: unknown) {
        if (!signal.aborted) {
          setState({ data: null, isLoading: false, error: err });
        }
      }
    },
    [sport, reloadToken],
  );

  const reload = useEventHandler(() => setReloadToken((t) => t + 1));

  return { ...state, reload };
}

interface State {
  data: GliderCatalog | null;
  isLoading: boolean;
  error: unknown;
}

const INITIAL_STATE: State = {
  data: null,
  isLoading: true,
  error: null,
};
