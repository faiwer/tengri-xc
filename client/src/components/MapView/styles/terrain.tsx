import type {
  LayerSpecification,
  SourceSpecification,
  StyleSpecification,
} from 'maplibre-gl';

export interface TerrainStyleOptions {
  /** `raster-dem` tiles template (maplibre-contour's shared-DEM protocol URL). */
  demTilesUrl: string;
  /** Contour vector tiles template (maplibre-contour's contour protocol URL). */
  contourTilesUrl: string;
  /** Min zoom of the DEM source. */
  minZoom: number;
  /** Max zoom of the DEM source. */
  maxZoom: number;
  /** Square DEM tile size emitted by the archive. */
  tileSize: number;
}

/**
 * Terrain style: OpenTopoMap base + hillshade + on-the-fly contour lines and
 * labels. The DEM and contours both come from one maplibre-contour source
 * backed by our `.tengri-map` archive, so hillshade and contours share a single
 * DEM fetch. Wired in {@link file://./terrainDem.ts}.
 */
export function buildTerrainStyle({
  demTilesUrl,
  contourTilesUrl,
  minZoom,
  maxZoom,
  tileSize,
}: TerrainStyleOptions): StyleSpecification {
  return {
    version: 8,
    name: 'tengri-terrain',
    sources: {
      opentopomap: OPEN_TOPO_MAP_SOURCE,
      terrainDem: genTengriDem({
        maxZoom,
        minZoom,
        tileSize,
        url: demTilesUrl,
      }),
      contours: genContoursSource({
        maxZoom,
        minZoom,
        url: contourTilesUrl,
      }),
    },
    layers: [
      BG_LAYER,
      (false as false) && OPEN_TOPO_MAP_LAYER,
      HILLSHADE_LAYER,
      CONTOUR_LINES_LAYER,
    ].filter((l) => !!l),
  };
}

const OPEN_TOPO_MAP_SOURCE: SourceSpecification = {
  type: 'raster',
  tiles: [
    'https://a.tile.opentopomap.org/{z}/{x}/{y}.png',
    'https://b.tile.opentopomap.org/{z}/{x}/{y}.png',
    'https://c.tile.opentopomap.org/{z}/{x}/{y}.png',
  ],
  tileSize: 256,
  minzoom: 0,
  maxzoom: 17,
  attribution:
    'Map data: © <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors, SRTM | Style: © <a href="https://opentopomap.org">OpenTopoMap</a> (CC-BY-SA)',
};

const OPEN_TOPO_MAP_LAYER: LayerSpecification = {
  id: 'opentopomap',
  type: 'raster',
  source: 'opentopomap',
  paint: { 'raster-opacity': 0.3 },
};

const genTengriDem = ({
  maxZoom,
  minZoom,
  url,
  tileSize,
}: {
  url: string;
  minZoom: number;
  maxZoom: number;
  tileSize: number;
}): SourceSpecification => ({
  type: 'raster-dem',
  tiles: [url],
  encoding: 'terrarium',
  tileSize,
  minzoom: minZoom,
  maxzoom: maxZoom,
});

const genContoursSource = ({
  maxZoom,
  minZoom,
  url,
}: {
  url: string;
  minZoom: number;
  maxZoom: number;
}): SourceSpecification => ({
  type: 'vector',
  tiles: [url],
  minzoom: minZoom,
  maxzoom: maxZoom,
});

const BG_LAYER: LayerSpecification = {
  id: 'background',
  type: 'background',
  paint: { 'background-color': '#FFFFFF' },
};

const HILLSHADE_LAYER: LayerSpecification = {
  id: 'tengri-hillshade',
  type: 'hillshade',
  source: 'terrainDem',
  paint: { 'hillshade-exaggeration': 0.3 },
};

const CONTOUR_LINES_LAYER: LayerSpecification = {
  id: 'contour-lines',
  type: 'line',
  source: 'contours',
  'source-layer': 'contours',
  paint: {
    // level=1 marks the major interval from the thresholds below.
    'line-color': ['match', ['get', 'level'], 1, '#555555', '#888888'],
    'line-opacity': 0.65,
    'line-width': ['match', ['get', 'level'], 1, 0.9, 0.45],
  },
};
