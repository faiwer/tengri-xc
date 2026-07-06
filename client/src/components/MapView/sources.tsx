import { type MapType } from './types';
import type { StyleSpecification } from 'maplibre-gl';

import { hybridStyle } from './styles/hybrid';
import { satelliteStyle } from './styles/satellite';
import { terrainStyle } from './styles/terrain';

const ROADMAP_STYLE_URL = 'https://tiles.openfreemap.org/styles/positron';

export const STYLE_BY_TYPE: Record<MapType, string | StyleSpecification> = {
  roadmap: ROADMAP_STYLE_URL,
  terrain: terrainStyle,
  satellite: satelliteStyle,
  hybrid: hybridStyle,
};
