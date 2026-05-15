import { useRef } from 'react';
import { useMap } from '@vis.gl/react-google-maps';
import { useAsyncEffect } from '../../core/hooks';

interface FitBoundsProps {
  bounds: google.maps.LatLngBoundsLiteral | null;
  skipInitialFit?: boolean;
  /** Pixels of inner padding inside the viewport when fitting. Default 32. */
  padding?: number;
}

/**
 * Smoothly fit the map to the given bounds whenever they change. Pass `null`
 * to do nothing (e.g. while the track is still loading).
 */
export function FitBounds({
  bounds,
  skipInitialFit = false,
  padding = 32,
}: FitBoundsProps) {
  const map = useMap();
  const shouldSkipInitialFit = useRef(skipInitialFit);
  const hasSeenBounds = useRef(false);

  useAsyncEffect(() => {
    if (!map || !bounds) {
      return;
    }

    if (shouldSkipInitialFit.current && !hasSeenBounds.current) {
      hasSeenBounds.current = true;
      return;
    }

    hasSeenBounds.current = true;

    map.fitBounds(bounds, padding);
  }, [map, bounds?.east, bounds?.north, bounds?.south, bounds?.west, padding]);

  return null;
}
