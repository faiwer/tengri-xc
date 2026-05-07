import type { Track } from './types';

export interface TrackPath {
  /** CSS color for this run; map renderer falls back to a default if absent. */
  color?: string;
  points: google.maps.LatLngLiteral[];
}

/**
 * Half-open `[from, to)` index range into a track. `to` may exceed `track.t`
 * length without harm; the projector clamps. Used to slice a single track
 * into colour-bucket runs (pre-flight / flight / post-flight, vario buckets,
 * etc.) without re-projecting points.
 */
export interface TrackWindow {
  /** First flying fix (inclusive). */
  takeoffIdx: number;
  /** First non-flying fix after the last flying fix (exclusive end of flight). */
  landedIdx: number;
}

const COLOR_PRE_POST = '#9ca3af';

/**
 * Project a `Track` into a list of polyline paths suitable for the map.
 *
 * Without a `window`, the entire track is returned as a single run with the
 * renderer's default colour. With a `window`, the track is sliced into up to
 * three runs:
 *
 *   1. `[0..takeoffIdx]` — pre-flight (gray), if non-empty.
 *   2. `[takeoffIdx..landedIdx]` — flight (default colour).
 *   3. `[landedIdx..n]` — post-flight (gray), if non-empty.
 *
 * Adjacent runs share a boundary point so the polyline is visually continuous
 * across the colour change.
 */
export function trackToPaths(track: Track, window?: TrackWindow): TrackPath[] {
  const fixCount = track.t.length;
  if (fixCount === 0) {
    return [];
  }

  if (!window) {
    return [{ points: projectRange(track, 0, fixCount) }];
  }

  const takeoff = clamp(window.takeoffIdx, 0, fixCount);
  const landed = clamp(window.landedIdx, takeoff, fixCount);
  const paths: TrackPath[] = [];

  if (takeoff > 0) {
    paths.push({
      color: COLOR_PRE_POST,
      points: projectRange(track, 0, takeoff + 1),
    });
  }

  if (landed > takeoff) {
    paths.push({ points: projectRange(track, takeoff, landed + 1) });
  }

  if (landed < fixCount - 1) {
    paths.push({
      color: COLOR_PRE_POST,
      points: projectRange(track, landed, fixCount),
    });
  }

  return paths;
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

const projectRange = (
  track: Track,
  from: number,
  to: number,
): google.maps.LatLngLiteral[] => {
  const len = to - from;
  const points: google.maps.LatLngLiteral[] = new Array(len);
  for (let i = 0; i < len; i++) {
    const j = from + i;
    points[i] = { lat: track.lat[j]! / 1e5, lng: track.lng[j]! / 1e5 };
  }
  return points;
};

const clamp = (v: number, min: number, max: number): number =>
  v < min ? min : v > max ? max : v;
