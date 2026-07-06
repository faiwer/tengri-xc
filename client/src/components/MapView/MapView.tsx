import { type Map as MapLibre } from '@vis.gl/react-maplibre';
import { type ReactNode, useMemo } from 'react';

import { MapCenterReporter } from './MapCenterReporter';
import styles from './MapView.module.scss';
import { MapTypeSwitcher } from './MapTypeSwitcher';
import { type MapType, MAP_TYPE_SCHEMA } from './types';
import { useMapHoverHandlers } from './useMapHoverHandlers';
import type { LatLng, LatLngBounds } from '../../utils/geo/coordinates';
import { useLocalStorageValue } from '../../utils/useLocalStorageValue';
import {
  DEFAULT_CENTER,
  DEFAULT_ZOOM,
  PADDING_PX,
  PREFETCH_BUFFER_PX,
} from './constants';
import { STYLE_BY_TYPE } from './sources';

export interface MapViewProps {
  /** Overlays rendered inside <Map>; they may use `useMap()` to attach. */
  children?: ReactNode;
  initialBounds?: LatLngBounds | null;
  initialPadding?: number;
  onCenterLatLng?: (point: LatLng) => void;
  onHoverLatLng?: (point: LatLng | null) => void;
  initialMapType?: MapType;
  hideControls?: boolean;
  lib: { Map: typeof MapLibre };
}

export function MapViewInternal({
  children,
  initialBounds,
  initialPadding = PADDING_PX,
  onCenterLatLng,
  onHoverLatLng,
  initialMapType: mapTypeInitial = 'terrain',
  hideControls = false,
  lib: { Map },
}: MapViewProps) {
  const { onMouseMove } = useMapHoverHandlers(onHoverLatLng);
  const [mapType, setMapType] = useLocalStorageValue('map-type', {
    schema: MAP_TYPE_SCHEMA,
    defaultValue: mapTypeInitial,
    strategy: 'initOnly',
  });
  const initialViewState = useInitialViewState(
    initialBounds ?? null,
    initialPadding,
  );

  return (
    <div
      className={styles.container}
      aria-label="Flight map"
      data-testid="flight-map"
    >
      {!hideControls && (
        <MapTypeSwitcher mapType={mapType} setMapType={setMapType} />
      )}
      {/* Wrap <Map/> with a div to render the map bigger than its container
      with negative offsets to load offscreen tiles. It has no option for this. */}
      <div className={styles.mapBuffer}>
        <Map
          mapStyle={STYLE_BY_TYPE[mapType]}
          initialViewState={initialViewState}
          style={{ width: '100%', height: '100%' }}
          dragRotate={false}
          touchPitch={false}
          onMouseMove={onMouseMove}
          // Retain ~16× the viewport tile count in cache (default 5×) so
          // pans back over recently-seen tiles paint from cache instead of
          // re-requesting them.
          maxTileCacheZoomLevels={16}
          // Keep partially-loaded tiles around during a zoom transition;
          // less abrupt detail pop-in, at the cost of a few extra requests.
          cancelPendingTileRequestsWhileZooming={false}
        >
          {onCenterLatLng && (
            <MapCenterReporter onCenterLatLng={onCenterLatLng} />
          )}
          {children}
        </Map>
      </div>
    </div>
  );
}

/**
 * Bounds drive the initial fit only; `<FitBounds>` handles re-fits after mount.
 */
function useInitialViewState(
  initialBounds: LatLngBounds | null,
  initialPadding: number,
): InitViewState {
  return useMemo(
    (): InitViewState =>
      initialBounds
        ? {
            bounds: [
              [initialBounds.west, initialBounds.south],
              [initialBounds.east, initialBounds.north],
            ],
            // Canvas is oversized by `PREFETCH_BUFFER_PX` on every side to
            // preload offscreen tiles.
            fitBoundsOptions: { padding: initialPadding + PREFETCH_BUFFER_PX },
          }
        : {
            longitude: DEFAULT_CENTER.lng,
            latitude: DEFAULT_CENTER.lat,
            zoom: DEFAULT_ZOOM,
          },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- initial values.
    [],
  );
}

type InitViewState = React.ComponentProps<typeof MapLibre>['initialViewState'];
