import { useState, useEffect, useError } from 'react';

import { LoadingIcon } from '../icons/LoadingIcon';

import { ErrorPane } from '../ErrorPane';
import { MapViewInternal, type MapViewProps } from './MapView';
import { createTerrainStyle } from './styles/terrainDem';
import styles from './MapView.module.scss';

export function MapView(props: Omit<MapViewProps, 'lib'>) {
  const [error, setError] = useState<unknown>(null);
  const [lib, setLib] = useState<MapViewProps['lib'] | null>(null);

  useError(setError);

  useEffect(() => {
    loadingPromise.then(setLib).catch(setError);
  }, []);

  return error ? (
    <ErrorPane error={error} />
  ) : lib ? (
    <MapViewInternal lib={lib} {...props} />
  ) : (
    <div className={styles.loading}>
      <LoadingIcon />
    </div>
  );
}

// Lazy load the map library and styles.
const loadingPromise = Promise.all([
  import('maplibre-gl/dist/maplibre-gl.css')
    // Delay to ensure the styles are applied.
    .then(() => new Promise((resolve) => setTimeout(resolve, 10))),
  import('react-map-gl/maplibre').then(({ Map }) => Map),
  import('maplibre-gl'),
  import('maplibre-contour').then((mod) => mod.default),
]).then(([_, Map, maplibre, mlcontour]): MapViewProps['lib'] => ({
  Map,
  // Registers maplibre-contour's DEM + contour protocols once, before any map
  // mounts, and returns the terrain style that references them.
  terrainStyle: createTerrainStyle(maplibre, mlcontour),
}));
