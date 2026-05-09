import { useRef, useState } from 'react';
import { trackError } from '../errors/trackError';
import { useEventHandler } from './useEventHandler';

/**
 * Wrap an async operation with loading + error state. Returns a
 * tuple, `useState`-style:
 *
 * - `run` — invoke `fn` with whatever args it takes; the resolved
 *   value flows through unchanged. Errors are exposed as `error`
 *   *and* rethrown so the caller can `await` it and react in the
 *   same closure if they want to.
 * - `isLoading` — `true` while the most recent `run` is in flight.
 *   Overlapping calls all execute, but only the *latest* one
 *   updates `isLoading` / `error` — earlier in-flight runs become
 *   stale on a new invocation and skip their setState calls so a
 *   slow first run can't flip a fresh start back to "done".
 * - `error` — the most recent rejection (`unknown`, since `fn` may
 *   throw anything). Reset to `null` on a successful run, *not* on
 *   an in-flight one, so a transient pending state doesn't blink an
 *   error banner away. Also routed through `trackError`.
 *
 * @example
 * const [submit, isLoading, error] = useAsync(async (values: FormValues) => {
 *   const me = await login(values);
 *   setMe(me);
 *   navigate(routes.flights());
 * });
 *
 * useErrorToast(error, { title: "Couldn't sign in" });
 *
 * <Form onFinish={submit} disabled={isLoading} />
 */
export function useAsync<Args extends unknown[], Ret>(
  fn: (...args: Args) => Promise<Ret>,
): [run: (...args: Args) => Promise<Ret>, isLoading: boolean, error: unknown] {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<unknown>(null);

  // Monotonic invocation counter. A run captures its id at entry
  // and only writes state if it's still the latest at resolution
  // time — that way overlapping calls don't fight over `isLoading`.
  const latestRunIdRef = useRef(0);

  const run = useEventHandler(async function useAsyncHandler(
    ...args: Args
  ): Promise<Ret> {
    latestRunIdRef.current += 1;
    const myRunId = latestRunIdRef.current;
    const isLatest = () => latestRunIdRef.current === myRunId;

    setIsLoading(true);
    try {
      const result = await fn(...args);
      if (isLatest()) {
        setError(null);
        setIsLoading(false);
      }
      return result;
    } catch (err: unknown) {
      if (isLatest()) {
        setError(err);
        setIsLoading(false);
      }
      trackError(err, { feature: 'unknown', origin: 'useAsync' });
      throw err;
    }
  });

  return [run, isLoading, error];
}
