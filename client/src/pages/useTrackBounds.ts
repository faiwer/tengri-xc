import { useMemo } from 'react';
import type { Track } from '../track';
import type { TrackWindow } from '../track/toPaths';
import {
  decimalDegree,
  E5_PER_DEGREE,
  type LatLngBounds,
} from '../utils/geo/coordinates';

/**
 * Bounding box for the flight window. Used both to fit the map and to size the
 * hover spatial grid, so those two behaviours agree on the same slice.
 */
export function useTrackBounds(
  track: Track | null,
  window?: TrackWindow,
): LatLngBounds | null {
  return useMemo(() => {
    if (!track || !window) {
      return null;
    }

    const fromIdx = window.takeoffIdx;
    const toIdx = window.landingIdx + 1;
    let south = Infinity;
    let north = -Infinity;
    let west = Infinity;
    let east = -Infinity;

    for (let idx = fromIdx; idx < toIdx; ++idx) {
      const lat = track.lat[idx] / E5_PER_DEGREE;
      const lng = track.lng[idx] / E5_PER_DEGREE;
      south = Math.min(south, lat);
      north = Math.max(north, lat);
      west = Math.min(west, lng);
      east = Math.max(east, lng);
    }

    return {
      south: decimalDegree(south),
      west: decimalDegree(west),
      north: decimalDegree(north),
      east: decimalDegree(east),
    };
  }, [track, window]);
}
