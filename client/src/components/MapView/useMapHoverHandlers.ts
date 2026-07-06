import type { MapLayerMouseEvent } from 'react-map-gl/maplibre';
import { useEffect, useRef } from 'react';
import { useEventHandler } from '../../core/hooks';
import { decimalDegree, type LatLng } from '../../utils/geo/coordinates';

export function useMapHoverHandlers(
  onHoverLatLng?: (point: LatLng | null) => void,
) {
  const frameRef = useRef<number | null>(null);

  // Mousemove is RAF-throttled; cancel a queued hover emit if the map unmounts.
  useEffect(
    () => () => {
      if (frameRef.current !== null) {
        cancelAnimationFrame(frameRef.current);
      }
    },
    [],
  );

  const onMouseMove = useEventHandler((event: MapLayerMouseEvent) => {
    if (!onHoverLatLng) {
      return;
    }

    if (frameRef.current !== null) {
      cancelAnimationFrame(frameRef.current);
    }

    const { lng, lat } = event.lngLat;
    frameRef.current = requestAnimationFrame(() => {
      frameRef.current = null;
      onHoverLatLng({ lat: decimalDegree(lat), lng: decimalDegree(lng) });
    });
  });

  return { onMouseMove };
}
