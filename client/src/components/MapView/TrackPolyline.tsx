import { useMap } from '@vis.gl/react-google-maps';
import { useEffect } from 'react';
import type { TrackPath } from '../../track/toPaths';

const DEFAULT_COLOR = '#dc2626';
const STROKE_WEIGHT = 3;
const STROKE_OPACITY = 0.9;

interface TrackPolylineProps {
  paths: readonly TrackPath[];
}

/**
 * Render one `google.maps.Polyline` per run. We pay one polyline object per
 * colour-bucket run, not per segment, so a typical paragliding track ends up
 * with at most a few hundred polylines even at high vario resolution.
 *
 * `@vis.gl/react-google-maps` doesn't ship a `<Polyline>` component, so we
 * attach imperatively via `useMap()` and tear down on unmount / paths change.
 */
export function TrackPolyline({ paths }: TrackPolylineProps) {
  const map = useMap();

  useEffect(() => {
    if (!map) return;

    const polylines = paths.map(
      (path) =>
        new google.maps.Polyline({
          path: path.points,
          map,
          strokeColor: path.color ?? DEFAULT_COLOR,
          strokeOpacity: STROKE_OPACITY,
          strokeWeight: STROKE_WEIGHT,
          clickable: false,
          zIndex: 10,
        }),
    );

    return () => {
      for (const pl of polylines) {
        pl.setMap(null);
      }
    };
  }, [map, paths]);

  return null;
}
