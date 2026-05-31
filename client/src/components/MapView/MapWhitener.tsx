import { useMap } from '@vis.gl/react-google-maps';
import { useEffect } from 'react';
import { MAP_Z_INDEX } from './zIndex';

/**
 * G-Maps provides limited possibilities to adjust the map colors. E.g., we have
 * no control over the colors of the terrain. By default, it's way too saturated.
 * So the track lines and other indicators are badly visible. To fight that we
 * whiten the map by overlaying a white rectangle on top of it.
 */
export function MapWhitener({ opacity }: { opacity: number }) {
  const map = useMap();

  useEffect(() => {
    if (!map) {
      return;
    }

    const rect = new google.maps.Rectangle({
      bounds: WORLD_BOUNDS,
      map,
      fillColor: '#ffffff',
      fillOpacity: opacity,
      strokeOpacity: 0,
      clickable: false,
      zIndex: MAP_Z_INDEX.whitener,
    });

    return () => {
      rect.setMap(null);
    };
  }, [map, opacity]);

  return null;
}

const WORLD_BOUNDS = {
  north: 85,
  south: -85,
  east: 180,
  west: -180,
};
