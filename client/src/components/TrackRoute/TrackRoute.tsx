import clsx from 'clsx';
import { useMemo } from 'react';
import { Layer, Marker, Source } from 'react-map-gl/maplibre';
import type { Route } from '../../api/tracks.io';
import styles from './TrackRoute.module.scss';
import { buildRouteGeometry } from './buildRouteGeometry';

interface TrackRouteProps {
  route: Route;
}

/**
 * Optimised route rendered over the raw flight track. Legs are MapLibre GL line
 * layers (need proper LineString rendering); turnpoints are DOM `<Marker>`s
 * (guarantees they paint above every canvas layer without fighting the deck.gl
 * overlay for z-order).
 */
export function TrackRoute({ route }: TrackRouteProps) {
  const { legs, waypoints } = useMemo(() => buildRouteGeometry(route), [route]);

  return (
    <>
      <Source id="tengri-route-legs" type="geojson" data={legs}>
        {solidLayer}
        {dashedLayer}
      </Source>

      {waypoints.map((point) => (
        <Marker
          key={`${point.lat},${point.lng}`}
          longitude={point.lng}
          latitude={point.lat}
        >
          <div className={clsx(styles.marker, styles.waypoint)} />
        </Marker>
      ))}
    </>
  );
}

const solidLayer = (
  <Layer
    id="tengri-route-legs-solid"
    type="line"
    filter={['==', ['get', 'style'], 'solid']}
    paint={{
      'line-color': ['get', 'color'],
      'line-width': 3,
    }}
  />
);

const dashedLayer = (
  <Layer
    id="tengri-route-legs-dashed"
    type="line"
    filter={['==', ['get', 'style'], 'dashed']}
    paint={{
      'line-color': ['get', 'color'],
      'line-width': 3,
      'line-dasharray': [2, 2],
    }}
  />
);
