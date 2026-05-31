import { useState } from 'react';
import type { Route, TrackMetadata } from '../api/tracks.io';
import { useAsyncEffect } from '../core/hooks';

export const useRoute = (metadata: TrackMetadata | null) => {
  const [selectedRoute, setSelectedRoute] = useState<Route | null>(null);

  useAsyncEffect(() => {
    const { mainRoute, routes } = metadata ?? {};
    if (!mainRoute || !routes?.length) {
      setSelectedRoute(null);
      return;
    }

    setSelectedRoute(routes.find((r) => r.id === mainRoute.id) ?? null);
  }, [metadata?.id]);

  return {
    selectedRoute,
    onRouteSelect: setSelectedRoute,
  };
};
