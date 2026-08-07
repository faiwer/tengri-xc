import { useState } from 'react';
import { useAsyncEffect } from './useAsyncEffect';

/**
 * The three mutually-exclusive states of a load, as a discriminated union:
 * loading (no data yet), settled with data, or failed. Narrowing on
 * `isLoading` / `error` gives you a non-null `data` in the success arm with no
 * extra guard.
 */
export type AsyncData<T> =
  | { data: null; isLoading: true; error: null }
  | { data: T; isLoading: false; error: null }
  | { data: null; isLoading: false; error: Error };

/**
 * Fetch-on-mount data loader: runs `fn` (handed an `AbortSignal`) whenever
 * `deps` change, and reports the load as an {@link AsyncData} union.
 *
 * Stale runs (superseded by a dep change / unmount, detected via
 * `signal.aborted`) write no state, so a slow earlier fetch can't clobber a
 * fresh one. Each run resets to the loading state. Deps aren't linted — see
 * {@link useAsyncEffect}; you own their correctness.
 *
 * @example
 * const { data: configs, isLoading, error } = useAsyncData(
 *   (signal) => getAdminOAuthProviders({ signal }),
 *   [],
 * );
 */
export function useAsyncData<T>(
  fn: (signal: AbortSignal) => Promise<T>,
  deps?: unknown[],
): AsyncData<T> {
  const [state, setState] = useState<AsyncData<T>>(LOADING);

  useAsyncEffect(async (signal) => {
    setState(LOADING);
    try {
      const data = await fn(signal);
      if (!signal.aborted) {
        setState({ data, isLoading: false, error: null });
      }
    } catch (err) {
      if (!signal.aborted) {
        setState({
          data: null,
          isLoading: false,
          error: err instanceof Error ? err : new Error(String(err)),
        });
      }
    }
  }, deps);

  return state;
}

const LOADING: AsyncData<never> = { data: null, isLoading: true, error: null };
