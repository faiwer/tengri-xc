import type { Track } from './types';
import type { VarioSegment } from './varioSegments/segments';

export interface TrackPath {
  /** CSS color for this run; map renderer falls back to a default if absent. */
  color?: string;
  /** Original track index for `points[0]`. */
  startIdx: number;
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
  landingIdx: number;
}

const COLOR_PRE_POST = '#9ca3af';

/**
 * Bucket-indexed colour ramp. Index 0 = bucket `-5`, index 10 = bucket `+5`.
 * Cool→warm spectrum: violet → blue → yellow → red, with `0` straddling the
 * yellow/light-blue divide. Green is omitted so the colour reads as a
 * continuous magnitude rather than as a categorical climb/sink signal.
 */
const VARIO_COLOR_RAMP = [
  '#7c3aed', // -5: violet-600
  '#6366f1', // -4: indigo-500
  '#3b82f6', // -3: blue-500
  '#38bdf8', // -2: sky-400
  '#7dd3fc', // -1: sky-300
  '#fde047', //  0: yellow-300
  '#facc15', // +1: yellow-400
  '#f59e0b', // +2: amber-500
  '#ea580c', // +3: orange-600
  '#dc2626', // +4: red-600
  '#991b1b', // +5: red-800
] as const;

/**
 * Project a `Track` into a list of polyline paths suitable for the map.
 *
 * Without a `window`, the entire track is returned as a single run with the
 * renderer's default colour. With a `window`, the track is sliced into up to
 * three runs:
 *
 *   1. `[0..takeoffIdx]` — pre-flight (gray), if non-empty.
 *   2. `[takeoffIdx..landingIdx]` — flight, optionally subdivided by `segments`.
 *   3. `[landingIdx..n]` — post-flight (gray), if non-empty.
 *
 * When `segments` is provided, the flight portion is split further into one
 * run per vario segment, coloured by 1 m/s bucket on a yellow→red (climb) /
 * light blue→violet (sink) ramp.
 *
 * Adjacent runs share a boundary point so the polyline is visually continuous
 * across each colour change.
 */
export function trackToPaths(
  track: Track,
  window?: TrackWindow,
  segments?: VarioSegment[],
): TrackPath[] {
  const fixCount = track.t.length;
  if (fixCount === 0) {
    return [];
  }

  if (!window) {
    return [projectPath(track, 0, fixCount)];
  }

  const takeoff = clamp(window.takeoffIdx, 0, fixCount);
  const landing = clamp(window.landingIdx, takeoff, fixCount);
  const paths: TrackPath[] = [];

  if (takeoff > 0) {
    paths.push(projectPath(track, 0, takeoff + 1, COLOR_PRE_POST));
  }

  if (landing > takeoff) {
    if (segments && segments.length > 0) {
      pushVarioRuns(paths, track, segments, fixCount);
    } else {
      paths.push(projectPath(track, takeoff, landing + 1));
    }
  }

  if (landing < fixCount - 1) {
    paths.push(projectPath(track, landing, fixCount, COLOR_PRE_POST));
  }

  return paths;
}

const pushVarioRuns = (
  paths: TrackPath[],
  track: Track,
  segments: VarioSegment[],
  fixCount: number,
): void => {
  for (const segment of segments) {
    const from = clamp(segment.startIdx, 0, fixCount);
    const to = clamp(segment.endIdx + 1, from, fixCount);
    if (to <= from) {
      continue;
    }
    paths.push(projectPath(track, from, to, colorForBucket(segment.bucket)));
  }
};

const colorForBucket = (bucket: number): string => {
  const idx = clamp(bucket + 5, 0, VARIO_COLOR_RAMP.length - 1);
  return VARIO_COLOR_RAMP[idx]!;
};

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

const projectPath = (
  track: Track,
  from: number,
  to: number,
  color?: string,
): TrackPath => {
  const len = to - from;
  const points: google.maps.LatLngLiteral[] = new Array(len);
  for (let i = 0; i < len; i++) {
    const j = from + i;
    points[i] = { lat: track.lat[j]! / 1e5, lng: track.lng[j]! / 1e5 };
  }
  return { color, startIdx: from, points };
};

const clamp = (v: number, min: number, max: number): number =>
  v < min ? min : v > max ? max : v;
