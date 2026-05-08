import { useMap } from '@vis.gl/react-google-maps';
import { useAsyncEffect } from '../../core/hooks';

interface FitBoundsProps {
  bounds: google.maps.LatLngBoundsLiteral | null;
  /** Pixels of inner padding inside the viewport when fitting. Default 32. */
  padding?: number;
}

/**
 * Smoothly fit the map to the given bounds whenever they change. Pass `null`
 * to do nothing (e.g. while the track is still loading).
 */
export function FitBounds({ bounds, padding = 32 }: FitBoundsProps) {
  const map = useMap();

  useAsyncEffect(() => {
    if (!map || !bounds) {
      return;
    }

    map.fitBounds(bounds, padding);
  }, [map, bounds?.east, bounds?.north, bounds?.south, bounds?.west, padding]);

  return null;
}
