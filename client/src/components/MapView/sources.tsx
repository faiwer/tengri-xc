import { type MapType } from './types';
import type { StyleSpecification } from 'maplibre-gl';

import { hybridStyle } from './styles/hybrid';
import { satelliteStyle } from './styles/satellite';

const ROADMAP_STYLE_URL = 'https://tiles.openfreemap.org/styles/positron';

/**
 * Style for a map type. `terrain` is built at load time (it needs
 * maplibre-contour's runtime protocol URLs), so it's passed in rather than
 * baked into a static table.
 */
export function styleFor(
  mapType: MapType,
  terrainStyle: StyleSpecification,
): string | StyleSpecification {
  switch (mapType) {
    case 'roadmap':
      return ROADMAP_STYLE_URL;
    case 'terrain':
      return terrainStyle;
    case 'satellite':
      return satelliteStyle;
    case 'hybrid':
      return hybridStyle;
  }
}
