import { useCallback, useLayoutEffect, useRef } from 'react';

/**
 * Returns a referentially-stable wrapper around `handler` that always
 * delegates to its latest version. Use it instead of `useCallback`
 * whenever the callback fires *outside* the render phase — DOM event
 * listeners, timers, IntersectionObserver callbacks, fetch `.then`
 * chains, anything async.
 *
 * Two wins over `useCallback`:
 *
 * 1. No dependency array. The wrapper is created once; you can't get
 *    "stale closure" bugs by forgetting a dep, and you can't churn
 *    consumers (effects, memoized children) by accidentally listing
 *    the wrong deps.
 * 2. The latest `handler` is captured via a ref written in
 *    `useLayoutEffect`, so the wrapper sees fresh props without the
 *    component itself re-rendering its consumers.
 *
 * **Do not call the returned function during render** — the latest
 * handler isn't installed until after commit, so a render-phase call
 * would silently invoke the previous render's closure.
 */
export function useEventHandler<Args extends unknown[], Ret>(
  handler: (...args: Args) => Ret,
): (...args: Args) => Ret {
  const handlerRef = useRef(handler);

  useLayoutEffect(() => {
    handlerRef.current = handler;
  }, [handler]);

  return useCallback((...args: Args) => handlerRef.current(...args), []);
}
