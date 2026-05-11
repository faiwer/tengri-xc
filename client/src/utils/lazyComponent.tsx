import {
  useState,
  type ComponentType,
  type ReactNode,
  type UnknownProps,
} from 'react';
import { trackError } from '../core/errors/trackError';
import { useAsyncEffect } from '../core/hooks';

type Loader<P extends UnknownProps> = () => Promise<ComponentType<P>>;

/**
 * Stand-in for `React.lazy` for builds where `<Suspense>` isn't supported.
 * Renders `fallback` until the loader resolves; from then on mounts the
 * resolved component synchronously (no flash).
 *
 * The loader returns the component directly, not an ESM module namespace —
 * keeping `{ default: ... }` out of the signature so call sites don't leak the
 * import shape. The usual recipe is:
 *
 * ```ts
 * const Markdown = lazyComponent(
 *   async () => (await import('./Markdown')).default,
 *   <Skeleton />,
 * );
 * ```
 *
 * The wrapped component accepts an extra optional `lazyFallback` prop that
 * overrides the per-instance fallback. Lets a single lazy export stay
 * encapsulated while callers swap the placeholder when they need to (e.g.
 * inline-sized loader vs full-page skeleton).
 *
 * Cache lives at the helper-call scope, so every instance of the returned
 * component shares one in-flight promise and one resolved value. Concurrent
 * first mounts won't double-fetch. Unmount mid-load cancels the per-instance
 * `setState`, not the import itself — the promise stays cached so the next
 * mount picks it up.
 *
 * On loader rejection: log via `trackError` and reset the cached promise so a
 * re-mount can retry.
 */
export function lazyComponent<P extends UnknownProps>(
  loader: Loader<P>,
  fallback: ReactNode,
  // `feature` is opt-in: identifies the call site in the error logs when the
  // import fails. Default fits the common case of "I'm splitting one component
  // out of the bundle".
  feature = 'lazyComponent',
): ComponentType<P & { lazyFallback?: ReactNode }> {
  let resolved: ComponentType<P> | null = null;
  let pending: Promise<ComponentType<P>> | null = null;

  const load = (): Promise<ComponentType<P>> => {
    if (resolved) {
      return Promise.resolve(resolved);
    }

    pending ??= loader().then(
      (component) => {
        resolved = component;
        return component;
      },
      (err) => {
        trackError(err, { feature, origin: 'lazyComponent' });
        // Clear so a re-mount can retry instead of awaiting the same
        // rejected promise forever.
        pending = null;
        throw err;
      },
    );
    return pending;
  };

  function Lazy(props: P & { lazyFallback?: ReactNode }) {
    const { lazyFallback, ...rest } = props;
    const [Component, setComponent] = useState<ComponentType<P> | null>(
      () => resolved,
    );

    useAsyncEffect(
      async (signal) => {
        if (Component) return;

        let component: ComponentType<P>;
        try {
          component = await load();
        } catch {
          // Already routed to `trackError` inside `load`; stay on the
          // fallback. Next mount re-runs `load` and may succeed.
          return;
        }
        if (!signal.aborted) setComponent(() => component);
      },
      [Component],
    );

    if (!Component) {
      return lazyFallback ?? fallback;
    }

    // `Omit<P & { lazyFallback?: … }, 'lazyFallback'>` doesn't reduce to
    // `P` for a generic `P`, but structurally `rest` is what `Component`
    // expects. Cast through `unknown` to bridge.
    return <Component {...(rest as unknown as P)} />;
  }

  Lazy.displayName = `Lazy(${feature})`;
  return Lazy;
}
