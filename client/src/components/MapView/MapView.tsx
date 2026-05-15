import { APIProvider, Map } from '@vis.gl/react-google-maps';
import { type ReactNode } from 'react';
import { MapCenterReporter } from './MapCenterReporter';
import styles from './MapView.module.scss';
import { useMapHoverHandlers } from './useMapHoverHandlers';

const API_KEY = import.meta.env.VITE_GOOGLE_MAPS_API_KEY;

// Greifenburg, Carinthia (Drautal). Zoom 10 fits the surrounding region.
const DEFAULT_CENTER = { lat: 46.751, lng: 13.1786 };
const DEFAULT_ZOOM = 10;

interface MapViewProps {
  /** Overlays rendered inside <Map>; they may use `useMap()` to attach. */
  children?: ReactNode;
  onCenterLatLng?: (point: google.maps.LatLngLiteral) => void;
  onHoverLatLng?: (point: google.maps.LatLngLiteral | null) => void;
}

export function MapView({
  children,
  onCenterLatLng,
  onHoverLatLng,
}: MapViewProps) {
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
          {onCenterLatLng && (
            <MapCenterReporter onCenterLatLng={onCenterLatLng} />
          )}
          {children}
        </Map>
      </APIProvider>
    </div>
  );
}
