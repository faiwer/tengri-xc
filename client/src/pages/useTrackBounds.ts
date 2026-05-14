import { useMemo } from 'react';
import type { Track } from '../track';
import type { TrackWindow } from '../track/toPaths';

/**
 * Bounding box for the flight window. Used both to fit the map and to size the
 * hover spatial grid, so those two behaviours agree on the same slice.
 */
export function useTrackBounds(
  track: Track | null,
  window?: TrackWindow,
): google.maps.LatLngBoundsLiteral | null {
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
      const lat = track.lat[idx]! / 1e5;
      const lng = track.lng[idx]! / 1e5;
      south = Math.min(south, lat);
      north = Math.max(north, lat);
      west = Math.min(west, lng);
      east = Math.max(east, lng);
    }

    return { south, west, north, east };
  }, [track, window]);
}
