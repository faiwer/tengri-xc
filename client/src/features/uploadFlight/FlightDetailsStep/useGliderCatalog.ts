import { useRef, useState } from 'react';
import { getMyGliderCatalog } from '../../../api/me/gliderCatalog';
import type { GliderCatalog, Sport } from '../../../api/admin/gliders.io';
import { useAsyncEffect, useErrorToast } from '../../../core/hooks';

/**
 * Load the brand + model catalog for `sport`, served from a per-mount cache
 * when already fetched. Toggling the kind switch back and forth shouldn't
 * refetch, but a fresh mount (modal reopen) starts clean.
 */
export function useGliderCatalog(sport: Sport) {
  const cacheRef = useRef<Map<Sport, GliderCatalog> | null>(null);
  cacheRef.current ??= new Map();
  const cache = cacheRef.current;

  const [catalog, setCatalog] = useState<GliderCatalog | null>(null);
  const [error, setError] = useState<unknown>(null);

  useErrorToast(error, { title: "Couldn't load gliders" });

  useAsyncEffect(
    async (signal) => {
      const cached = cache.get(sport);
      if (cached) {
        setCatalog(cached);
        setError(null);
        return;
      }

      setCatalog(null);
      setError(null);
      try {
        const fetched = await getMyGliderCatalog(sport, { signal });
        cache.set(sport, fetched);

        if (signal.aborted) {
          return;
        }
        setCatalog(fetched);
      } catch (err) {
        if (!signal.aborted) {
          setError(err);
        }
      }
    },
    [sport],
  );

  return {
    catalog,
    isLoading: catalog === null && error === null,
    error,
  };
}
