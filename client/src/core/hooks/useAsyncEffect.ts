import { useEffect, useLayoutEffect } from 'react';
import { trackError } from '../errors/trackError';

type AsyncEffectCallback = (
  signal: AbortSignal,
) => Promise<void> | (() => void) | void;

/**
 * `useEffect` for async work. Three things it gives you over the bare
 * hook:
 *
 * 1. The callback may be `async`. React's own `useEffect` insists the
 *    cleanup return be a function or `undefined`; an `async` body
 *    returns a `Promise`, which silently breaks cleanup.
 * 2. An `AbortSignal` is supplied to the callback and `.abort()`'d on
 *    teardown, so any `fetch` (or our own `apiGet`) plumbed through it
 *    stops on unmount / dep change.
 * 3. Unhandled rejections are routed to {@link trackError} instead of
 *    becoming a console-only `Uncaught (in promise)`. `AbortError` is
 *    swallowed because it's the cleanup signal *we* fired.
 *
 * Sync usage stays available — return a cleanup function and it's
 * called alongside `controller.abort()` on teardown.
 *
 * **Note on deps**: this hook accepts `unknown[]` and is *not* watched
 * by `react-hooks/exhaustive-deps`. That's deliberate (the async
 * callback closes over things eslint can't see through), but it means
 * you own dep correctness. Be honest about it.
 *
 * @example
 * useAsyncEffect(async (signal) => {
 *   const data = await getThing(id, { signal });
 *   if (!signal.aborted) setData(data);
 * }, [id]);
 *
 * @example
 * useAsyncEffect(() => {
 *   const sub = subscribe(onMessage);
 *   return () => sub.unsubscribe();
 * }, []);
 */
export function useAsyncEffect(
  fn: AsyncEffectCallback,
  deps?: unknown[],
): void {
  useAsyncEffectImpl(fn, useEffect, deps);
}

/** {@link useAsyncEffect}, but runs synchronously after DOM mutations. */
export function useAsyncLayoutEffect(
  fn: AsyncEffectCallback,
  deps?: unknown[],
): void {
  useAsyncEffectImpl(fn, useLayoutEffect, deps);
}

function useAsyncEffectImpl(
  fn: AsyncEffectCallback,
  useEffectHook: typeof useEffect | typeof useLayoutEffect,
  deps?: unknown[],
): void {
  useEffectHook(() => {
    const controller = new AbortController();
    const result = fn(controller.signal);

    // Detect the async branch via duck-typing on the return value.
    // Checking `instanceof Promise` would miss promise-likes from other
    // realms (iframes, polyfills); a `then` probe is what React itself
    // does for the same reason.
    if (
      result !== undefined &&
      typeof result === 'object' &&
      'then' in result
    ) {
      result.catch((err: unknown) => {
        // Aborts are *our* cleanup signal — never a real error.
        if (err instanceof DOMException && err.name === 'AbortError') {
          return;
        }
        trackError(err, { feature: 'unknown', origin: 'useAsyncEffect' });
      });
    }

    return () => {
      controller.abort();
      if (typeof result === 'function') {
        result();
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
