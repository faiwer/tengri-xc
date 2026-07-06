import type { Track } from '../track';
import type { TrackWindow } from '../track/toPaths';
import {
  decimalDegree,
  E5_PER_DEGREE,
  type LatLng,
  type LatLngBounds,
} from '../utils/geo/coordinates';

export interface SpatialIndex {
  cellSize: number;
  fromIdx: number;
  maxCellX: number;
  maxCellY: number;
  minLat: number;
  minLng: number;
  buckets: Map<string, number[]>;
}

/** Spatial grid resolution over the track bounds. Higher is more precise. */
const GRID_CELLS_PER_LONG_AXIS = 200;

// O(n). Calculated once.
export const buildSpatialIndex = (
  track: Track,
  window: TrackWindow,
  bounds: LatLngBounds,
): SpatialIndex => {
  const fromIdx = window.takeoffIdx;
  const toIdx = window.landingIdx + 1;
  const minLat = bounds.south;
  const minLng = bounds.west;
  const latSpan = bounds.north - bounds.south;
  const lngSpan = bounds.east - bounds.west;
  const cellSize = Math.max(latSpan, lngSpan) / GRID_CELLS_PER_LONG_AXIS || 1;
  const maxCellX = Math.max(0, Math.floor(lngSpan / cellSize));
  const maxCellY = Math.max(0, Math.floor(latSpan / cellSize));
  const buckets = new Map<string, number[]>();

  for (let idx = fromIdx; idx < toIdx; idx++) {
    const cell = cellForPoint(
      {
        lat: decimalDegree(track.lat[idx]! / E5_PER_DEGREE),
        lng: decimalDegree(track.lng[idx]! / E5_PER_DEGREE),
      },
      { cellSize, minLat, minLng, maxCellX, maxCellY },
    );
    const key = cellKey(cell.x, cell.y);
    const bucket = buckets.get(key);
    if (bucket) {
      bucket.push(idx);
    } else {
      buckets.set(key, [idx]);
    }
  }

  return {
    cellSize,
    fromIdx,
    maxCellX,
    maxCellY,
    minLat,
    minLng,
    buckets,
  };
};

/**
 * Typical: O(k), where k is the number of points in nearby buckets.
 */
export const nearestTrackIndex = (
  track: Track,
  index: SpatialIndex,
  point: LatLng,
): number | null => {
  const cell = cellForPoint(point, index);
  let bestIdx: number | null = null;
  let bestDistance = Infinity;

  const maxRadius = Math.max(index.maxCellX, index.maxCellY);

  for (let radius = 0; radius <= maxRadius; radius++) {
    const searchDistance = radius * index.cellSize;
    if (bestIdx !== null && searchDistance * searchDistance > bestDistance) {
      return bestIdx;
    }

    for (let y = cell.y - radius; y <= cell.y + radius; y++) {
      for (let x = cell.x - radius; x <= cell.x + radius; x++) {
        if (
          x < 0 ||
          y < 0 ||
          x > index.maxCellX ||
          y > index.maxCellY ||
          (radius > 0 &&
            x > cell.x - radius &&
            x < cell.x + radius &&
            y > cell.y - radius &&
            y < cell.y + radius)
        ) {
          continue;
        }

        const bucket = index.buckets.get(cellKey(x, y));
        if (!bucket) {
          continue;
        }

        for (const idx of bucket) {
          const distance = squaredDistance(point, {
            lat: decimalDegree(track.lat[idx]! / E5_PER_DEGREE),
            lng: decimalDegree(track.lng[idx]! / E5_PER_DEGREE),
          });
          if (distance < bestDistance) {
            bestDistance = distance;
            bestIdx = idx;
          }
        }
      }
    }
  }

  return bestIdx;
};

const cellForPoint = (
  point: LatLng,
  index: Pick<
    SpatialIndex,
    'cellSize' | 'maxCellX' | 'maxCellY' | 'minLat' | 'minLng'
  >,
): { x: number; y: number } => ({
  x: clamp(
    Math.floor((point.lng - index.minLng) / index.cellSize),
    0,
    index.maxCellX,
  ),
  y: clamp(
    Math.floor((point.lat - index.minLat) / index.cellSize),
    0,
    index.maxCellY,
  ),
});

const cellKey = (x: number, y: number): string => `${x}:${y}`;

const squaredDistance = (a: LatLng, b: LatLng): number => {
  const dx = a.lng - b.lng;
  const dy = a.lat - b.lat;
  return dx * dx + dy * dy;
};

const clamp = (value: number, min: number, max: number): number =>
  value < min ? min : value > max ? max : value;
