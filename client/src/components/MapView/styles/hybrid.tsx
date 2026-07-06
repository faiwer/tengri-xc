import type { StyleSpecification } from 'maplibre-gl';

export const hybridStyle: StyleSpecification = {
  version: 8,
  name: 'tengri-hybrid',
  sources: {
    'esri-world-imagery': {
      type: 'raster',
      tiles: [
        'https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}',
      ],
      tileSize: 256,
      minzoom: 0,
      maxzoom: 19,
      attribution:
        'Sources: Esri, Maxar, Earthstar Geographics, and the GIS User Community',
    },
    'esri-reference': {
      type: 'raster',
      tiles: [
        'https://server.arcgisonline.com/ArcGIS/rest/services/Reference/World_Boundaries_and_Places/MapServer/tile/{z}/{y}/{x}',
      ],
      tileSize: 256,
      minzoom: 0,
      maxzoom: 19,
      attribution: 'Labels: Esri',
    },
  },
  layers: [
    {
      id: 'esri-world-imagery',
      type: 'raster',
      source: 'esri-world-imagery',
    },
    {
      id: 'esri-reference',
      type: 'raster',
      source: 'esri-reference',
    },
  ],
};
