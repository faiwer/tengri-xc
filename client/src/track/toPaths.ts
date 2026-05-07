import type { Track } from './types';

export interface TrackPath {
  /** CSS color for this run; map renderer falls back to a default if absent. */
  color?: string;
  points: google.maps.LatLngLiteral[];
}

/**
 * Project a `Track` into a list of polyline paths suitable for the map.
 *
 * Today: one run, the entire track. The shape is intentionally a *list* so
 * vario-coloured rendering can later split the same track into many runs of
 * same-bucket segments without changing the renderer.
 */
export function trackToPaths(track: Track): TrackPath[] {
  const n = track.t.length;
  const points: google.maps.LatLngLiteral[] = new Array(n);
  for (let i = 0; i < n; i++) {
    points[i] = { lat: track.lat[i]! / 1e5, lng: track.lng[i]! / 1e5 };
  }
  return [{ points }];
}

/**
 * Bounding box for a list of paths. Used to fit the camera to the data.
 * Returns `null` when there are no points.
 */
export function pathsBounds(
  paths: readonly TrackPath[],
): google.maps.LatLngBoundsLiteral | null {
  let minLat = Infinity;
  let maxLat = -Infinity;
  let minLng = Infinity;
  let maxLng = -Infinity;
  let any = false;

  for (const path of paths) {
    for (const p of path.points) {
      any = true;
      if (p.lat < minLat) minLat = p.lat;
      if (p.lat > maxLat) maxLat = p.lat;
      if (p.lng < minLng) minLng = p.lng;
      if (p.lng > maxLng) maxLng = p.lng;
    }
  }

  if (!any) return null;
  return { south: minLat, west: minLng, north: maxLat, east: maxLng };
}
