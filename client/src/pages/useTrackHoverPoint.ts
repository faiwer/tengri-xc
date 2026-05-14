import { useMemo, useState } from 'react';
import type { Track } from '../track';
import type { TrackWindow } from '../track/toPaths';

/**
 * Convert a chart hover fraction into an approximate point inside the flight
 * window. The chart data may be bucketed, so the marker follows flight progress
 * rather than an exact timestamp.
 */
export function useTrackHoverPoint(
  track: Track | null,
  window?: TrackWindow,
): {
  point: google.maps.LatLngLiteral | null;
  setHoverFraction: (fraction: number | null) => void;
} {
  const [hoverFraction, setHoverFraction] = useState<number | null>(null);

  const point = useMemo<google.maps.LatLngLiteral | null>(() => {
    if (!track || !window || hoverFraction === null) {
      return null;
    }

    const idx = Math.round(
      window.takeoffIdx +
        hoverFraction * (window.landingIdx - window.takeoffIdx),
    );

    return {
      lat: track.lat[idx]! / 1e5,
      lng: track.lng[idx]! / 1e5,
    };
  }, [track, window, hoverFraction]);

  return { point, setHoverFraction };
}
