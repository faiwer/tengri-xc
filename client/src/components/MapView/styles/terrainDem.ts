import type { AddProtocolAction, StyleSpecification } from 'maplibre-gl';

import { createDemLoader } from '../demSource';
import { buildTerrainStyle } from './terrain';

/**
 * Build the terrain style and register maplibre-contour's DEM + contour
 * protocols. maplibre-contour fetches DEM tiles with its own plain-`fetch`
 * manager, which can't speak `tengri://`, so we swap the manager's `getTile`
 * for our archive-backed loader. Hillshade and contours then share that one DEM
 * source. `worker: false` is required: `getTile` is a closure and can't cross
 * into a web worker.
 */
export function createTerrainStyle(
  maplibre: { addProtocol(id: string, action: AddProtocolAction): void },
  mlcontour: Mlcontour,
): StyleSpecification {
  const loader = createDemLoader();

  const demSource = new mlcontour.DemSource({
    // A pseudo URL to fulfill the protocol handler.
    url: 'tengri-terrain-dem://{z}/{x}/{y}',
    encoding: 'terrarium',
    maxzoom: loader.maxZoom,
    worker: false,
    cacheSize: 200,
  });

  const manager = demSource.manager as LocalDemManager;
  manager.getTile = async (url, abortController) => {
    const [z, x, y] = demSource.parseUrl(url);
    const data = await loader.loadTerrariumTile(
      z,
      x,
      y,
      abortController.signal,
    );
    return { data: new Blob([data], { type: 'image/png' }) };
  };

  demSource.setupMaplibre(maplibre);

  return buildTerrainStyle({
    demTilesUrl: demSource.sharedDemProtocolUrl,
    contourTilesUrl: demSource.contourProtocolUrl({
      // 1 means — keep the value intact. TODO: support converting to feet.
      multiplier: 1, // metres
      thresholds: CONTOUR_THRESHOLDS,
      contourLayer: 'contours',
      // Field name for the numerical elevation value. Used in styles.
      elevationKey: 'ele',
      // Field name for the contour level (major/minor). Used in styles.
      levelKey: 'level',
      // MVT standard, 4096x4096 grid.
      extent: 4096,
      // How many pixels to generate on each tile into the neighboring tile to
      // reduce rendering artifacts
      buffer: 1,
    }),
    minZoom: loader.minZoom,
    maxZoom: loader.maxZoom,
    tileSize: loader.tileSize,
  });
}

type Mlcontour = typeof import('maplibre-contour').default;
type LocalDemManager = InstanceType<Mlcontour['LocalDemManager']>;

/** Zoom → `[minor, major]` contour interval in metres. */
const CONTOUR_THRESHOLDS = {
  8: [200, 1000],
  10: [100, 500],
  12: [50, 250],
  14: [25, 100],
};
