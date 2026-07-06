import { MapboxOverlay } from '@deck.gl/mapbox';
import { PathLayer } from '@deck.gl/layers';
import { useEffect } from 'react';
import { useMap } from 'react-map-gl/maplibre';
import type { TrackPath } from '../../track/toPaths';
import { hexToRgba } from '../../utils/colors';

const DEFAULT_COLOR = '#dc2626';
const STROKE_WEIGHT = 3;
const STROKE_OPACITY = 0.9;

interface TrackPolylineProps {
  paths: readonly TrackPath[];
}

/**
 * Render the track as a single deck.gl `PathLayer`. One feature per
 * colour-bucket run keeps the layer small (a few hundred polylines at most for
 * a typical hg track) while the GPU draws the segments.
 */
export function TrackPolyline({ paths }: TrackPolylineProps) {
  const map = useMap().current?.getMap();

  useEffect(() => {
    if (!map) {
      return;
    }

    const data: DeckTrackPath[] = paths.map((trackPath) => ({
      path: trackPath.points.map((point) => [point.lng, point.lat]),
      color: hexToRgba(trackPath.color ?? DEFAULT_COLOR, STROKE_OPACITY),
    }));

    const layer = new PathLayer<DeckTrackPath>({
      id: 'track',
      data,
      getPath: (d) => d.path,
      getColor: (d) => d.color,
      getWidth: STROKE_WEIGHT,
      widthUnits: 'pixels',
    });
    const overlay = new MapboxOverlay({
      interleaved: true, // Render in the MapLibre canvas.
      layers: [layer],
    });
    map.addControl(overlay);

    return () => {
      map.removeControl(overlay);
    };
  }, [map, paths]);

  return null;
}

type DeckTrackPath = {
  path: [number, number][];
  color: [number, number, number, number];
};
