import { type MapMouseEvent } from '@vis.gl/react-google-maps';
import { useEffect, useRef } from 'react';
import { useEventHandler } from '../../core/hooks';

export function useMapHoverHandlers(
  onHoverLatLng?: (point: google.maps.LatLngLiteral | null) => void,
) {
  const frameRef = useRef<number | null>(null);
  const emitHover = useEventHandler(
    (point: google.maps.LatLngLiteral | null) => {
      onHoverLatLng?.(point);
    },
  );

  // Mousemove is RAF-throttled; cancel a queued hover emit if the map unmounts.
  useEffect(
    () => () => {
      if (frameRef.current !== null) {
        cancelAnimationFrame(frameRef.current);
      }
    },
    [],
  );

  const onMousemove = useEventHandler((event: MapMouseEvent) => {
    if (!onHoverLatLng) {
      return;
    }

    if (frameRef.current !== null) {
      cancelAnimationFrame(frameRef.current);
    }

    const point = event.detail.latLng ?? null;
    frameRef.current = requestAnimationFrame(() => {
      frameRef.current = null;
      emitHover(point);
    });
  });

  return { onMousemove };
}
