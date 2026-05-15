import {
  APIProvider,
  Map,
  type MapMouseEvent,
} from '@vis.gl/react-google-maps';
import { useEffect, useRef, type ReactNode } from 'react';
import { useEventHandler } from '../../core/hooks';
import styles from './MapView.module.scss';

const API_KEY = import.meta.env.VITE_GOOGLE_MAPS_API_KEY;

// Greifenburg, Carinthia (Drautal). Zoom 10 fits the surrounding region.
const DEFAULT_CENTER = { lat: 46.751, lng: 13.1786 };
const DEFAULT_ZOOM = 10;

interface MapViewProps {
  /** Overlays rendered inside <Map>; they may use `useMap()` to attach. */
  children?: ReactNode;
  onHoverLatLng?: (point: google.maps.LatLngLiteral | null) => void;
}

export function MapView({ children, onHoverLatLng }: MapViewProps) {
  const { onMousemove } = useMapHoverHandlers(onHoverLatLng);

  return (
    <div className={styles.container}>
      <APIProvider apiKey={API_KEY}>
        <Map
          className={styles.map}
          defaultCenter={DEFAULT_CENTER}
          defaultZoom={DEFAULT_ZOOM}
          gestureHandling="greedy"
          disableDefaultUI
          fullscreenControl
          mapTypeControl
          onMousemove={onMousemove}
        >
          {children}
        </Map>
      </APIProvider>
    </div>
  );
}

function useMapHoverHandlers(
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
