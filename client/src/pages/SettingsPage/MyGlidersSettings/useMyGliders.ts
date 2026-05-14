import { useState } from 'react';

import { listMyGliders } from '../../../api/me/gliders';
import type { MyGlider } from '../../../api/me/gliders.io';
import {
  useAsyncEffect,
  useErrorToast,
  useEventHandler,
} from '../../../core/hooks';

/**
 * Loads the signed-in pilot's gliders and exposes a `reload()` to re-fetch
 * (used after a successful delete). Errors surface via `useErrorToast` *and*
 * the returned `error` field — the page picks what to render.
 */
export function useMyGliders() {
  const [state, setState] = useState<State>(INITIAL_STATE);
  const [reloadToken, setReloadToken] = useState(0);

  useErrorToast(state.error, { title: "Couldn't load your gliders" });

  useAsyncEffect(
    async (signal) => {
      setState((prev) => ({ ...prev, isLoading: true, error: null }));
      try {
        const data = await listMyGliders({ signal });
        if (!signal.aborted) {
          setState({ data, isLoading: false, error: null });
        }
      } catch (err: unknown) {
        if (!signal.aborted) {
          setState((prev) => ({
            data: prev.data,
            isLoading: false,
            error: err,
          }));
        }
      }
    },
    [reloadToken],
  );

  const reload = useEventHandler(() => setReloadToken((t) => t + 1));

  return { ...state, reload };
}

interface State {
  data: MyGlider[] | null;
  isLoading: boolean;
  error: unknown;
}

const INITIAL_STATE: State = {
  data: null,
  isLoading: true,
  error: null,
};
