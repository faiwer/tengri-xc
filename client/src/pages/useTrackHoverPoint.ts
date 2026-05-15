import { useMemo, useState } from 'react';
import { useEventHandler } from '../core/hooks';
import type { Track } from '../track';
import type { TrackWindow } from '../track/toPaths';
import { buildSpatialIndex, nearestTrackIndex } from './trackHoverSpatialIndex';

interface HoverPointState {
  point: google.maps.LatLngLiteral | null;
  trackIndex: number | null;
  /**
   * External chart cursor control: number = map drives the chart cursor,
   * null = clear the chart cursor, undefined = chart owns its native hover.
   */
  chartHoverFraction: number | null | undefined;
  clearHover: () => void;
  setHoverFraction: (fraction: number | null) => void;
  setHoverLatLng: (point: google.maps.LatLngLiteral | null) => void;
}

type HoverSource =
  | { kind: 'none' }
  | { kind: 'chart'; fraction: number }
  | { kind: 'map'; point: google.maps.LatLngLiteral };

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
  const [hoverSource, setHoverSource] = useState<HoverSource>({ kind: 'none' });
  const hoverLatLng = hoverSource.kind === 'map' ? hoverSource.point : null;
  const hoverFraction =
    hoverSource.kind === 'chart' ? hoverSource.fraction : null;

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

    const idx = hoverTrackIndex ?? fractionToTrackIndex(window, hoverFraction);

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

  const trackIndex =
    hoverSource.kind === 'map'
      ? hoverTrackIndex
      : fractionToTrackIndex(window, hoverFraction);
  const chartHoverFraction =
    hoverSource.kind === 'map'
      ? mapHoverFraction
      : hoverSource.kind === 'none'
        ? null
        : undefined;
  const clearHover = useEventHandler(() => {
    setHoverSource({ kind: 'none' });
  });
  const setHoverFraction = useEventHandler((fraction: number | null) => {
    if (fraction !== null) {
      setHoverSource({ kind: 'chart', fraction });
    }
  });
  const setHoverLatLng = useEventHandler(
    (latLng: google.maps.LatLngLiteral | null) => {
      if (latLng !== null) {
        setHoverSource({ kind: 'map', point: latLng });
      }
    },
  );

  return {
    point,
    trackIndex,
    chartHoverFraction,
    clearHover,
    setHoverFraction,
    setHoverLatLng,
  };
}

const fractionToTrackIndex = (
  window: TrackWindow | undefined,
  fraction: number | null,
): number | null =>
  window && fraction !== null
    ? Math.round(
        window.takeoffIdx + fraction * (window.landingIdx - window.takeoffIdx),
      )
    : null;

const clamp = (value: number, min: number, max: number): number =>
  value < min ? min : value > max ? max : value;
