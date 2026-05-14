import { useMemo, useState } from 'react';
import type { Track } from '../track';
import type { TrackWindow } from '../track/toPaths';
import { buildSpatialIndex, nearestTrackIndex } from './trackHoverSpatialIndex';

interface HoverPointState {
  point: google.maps.LatLngLiteral | null;
  mapHoverFraction: number | null;
  setHoverFraction: (fraction: number | null) => void;
  setHoverLatLng: (point: google.maps.LatLngLiteral | null) => void;
}

/**
 * Convert chart and map hover state into one marker point. Chart data may be
 * bucketed, so chart hover follows flight progress; map hover uses a coarse
 * spatial grid over the flight fixes to find the nearest point quickly.
 */
export function useTrackHoverPoint(
  track: Track | null,
  window?: TrackWindow,
  bounds?: google.maps.LatLngBoundsLiteral | null,
): HoverPointState {
  const [hoverFraction, setHoverFraction] = useState<number | null>(null);
  const [hoverLatLng, setHoverLatLng] =
    useState<google.maps.LatLngLiteral | null>(null);

  const spatialIndex = useMemo(
    () =>
      track && window && bounds
        ? buildSpatialIndex(track, window, bounds)
        : null,
    [track, window, bounds],
  );

  const hoverTrackIndex = useMemo((): number | null => {
    if (!track || !spatialIndex || !hoverLatLng) {
      return null;
    }

    return nearestTrackIndex(track, spatialIndex, hoverLatLng);
  }, [track, spatialIndex, hoverLatLng]);

  const point = useMemo<google.maps.LatLngLiteral | null>(() => {
    if (!track) {
      return null;
    }

    const idx =
      hoverTrackIndex ??
      (window && hoverFraction !== null
        ? Math.round(
            window.takeoffIdx +
              hoverFraction * (window.landingIdx - window.takeoffIdx),
          )
        : null);

    if (idx === null) {
      return null;
    }

    return {
      lat: track.lat[idx]! / 1e5,
      lng: track.lng[idx]! / 1e5,
    };
  }, [track, window, hoverFraction, hoverTrackIndex]);

  const mapHoverFraction = useMemo(() => {
    if (!window || hoverTrackIndex === null) {
      return null;
    }

    const span = window.landingIdx - window.takeoffIdx;
    if (span <= 0) {
      return null;
    }

    return clamp((hoverTrackIndex - window.takeoffIdx) / span, 0, 1);
  }, [window, hoverTrackIndex]);

  return { point, mapHoverFraction, setHoverFraction, setHoverLatLng };
}

const clamp = (value: number, min: number, max: number): number =>
  value < min ? min : value > max ? max : value;
