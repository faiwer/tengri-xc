import { APIProvider, Map } from '@vis.gl/react-google-maps';
import { type ReactNode } from 'react';
import { MapCenterReporter } from './MapCenterReporter';
import PALE_THEME from './paleTheme.json' with { type: 'json' };
import { MapWhitener } from './MapWhitener';
import styles from './MapView.module.scss';
import { useMapHoverHandlers } from './useMapHoverHandlers';
import { useLocalStorageValue } from '../../utils/useLocalStorageValue';
import { MAP_TYPE_SCHEMA, type MapType } from './types';
import { MapTypeSwitcher } from './MapTypeSwitcher';

const API_KEY = import.meta.env.VITE_GOOGLE_MAPS_API_KEY;

// Greifenburg, Carinthia (Drautal). Zoom 10 fits the surrounding region.
const DEFAULT_CENTER = { lat: 46.751, lng: 13.1786 };
const DEFAULT_ZOOM = 10;
const PADDING = 32;

interface MapViewProps {
  /** Overlays rendered inside <Map>; they may use `useMap()` to attach. */
  children?: ReactNode;
  initialBounds?: google.maps.LatLngBoundsLiteral | null;
  onCenterLatLng?: (point: google.maps.LatLngLiteral) => void;
  onHoverLatLng?: (point: google.maps.LatLngLiteral | null) => void;
  initialMapType?: MapType;
}

export function MapView({
  children,
  initialBounds,
  onCenterLatLng,
  onHoverLatLng,
  initialMapType: mapTypeInitial = 'terrain',
}: MapViewProps) {
  const { onMousemove } = useMapHoverHandlers(onHoverLatLng);
  const [mapType, setMapType] = useLocalStorageValue('map-type', {
    schema: MAP_TYPE_SCHEMA,
    defaultValue: mapTypeInitial,
    strategy: 'initOnly',
  });

  return (
    <div className={styles.container}>
      <MapTypeSwitcher mapType={mapType} setMapType={setMapType} />
      <APIProvider apiKey={API_KEY}>
        <Map
          mapTypeId={mapType}
          className={styles.map}
          defaultCenter={DEFAULT_CENTER}
          defaultZoom={DEFAULT_ZOOM}
          defaultBounds={
            initialBounds ? { ...initialBounds, padding: PADDING } : undefined
          }
          gestureHandling="greedy"
          disableDefaultUI
          fullscreenControl
          styles={mapType === 'terrain' ? PALE_THEME : undefined}
          onMousemove={onMousemove}
        >
          {(mapType === 'terrain' || mapType === 'roadmap') && (
            <MapWhitener opacity={mapType === 'terrain' ? 0.7 : 0.3} />
          )}
          {onCenterLatLng && (
            <MapCenterReporter onCenterLatLng={onCenterLatLng} />
          )}
          {children}
        </Map>
      </APIProvider>
    </div>
  );
}
