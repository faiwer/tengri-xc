import { useRef } from 'react';
import { useMap } from 'react-map-gl/maplibre';
import { useAsyncEffect } from '../../core/hooks';
import type { LatLngBounds } from '../../utils/geo/coordinates';
import { PREFETCH_BUFFER_PX } from './constants';

interface FitBoundsProps {
  bounds: LatLngBounds | null;
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
  const map = useMap().current?.getMap();
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

    map.fitBounds(
      [
        [bounds.west, bounds.south],
        [bounds.east, bounds.north],
      ],
      // Canvas overhangs the visible container by `PREFETCH_BUFFER_PX`; add
      // it back so the requested inset is measured from the *visible* edge.
      { padding: padding + PREFETCH_BUFFER_PX },
    );
  }, [map, bounds?.east, bounds?.north, bounds?.south, bounds?.west, padding]);

  return null;
}
