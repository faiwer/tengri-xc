import { useState } from 'react';
import type { LngLat } from 'tengri-maplibre';
import { useAsyncEffect } from '../core/hooks';
import { createDemLoader } from '../components/MapView/demSource';
import type { Track } from '../track';
import type { TrackWindow } from '../track/toPaths';
import { E5_PER_DEGREE } from '../utils/geo/coordinates';

export interface GroundElevations {
  /**
   * Ground elevation in metres for every fix in `[takeoffIdx, landingIdx + 1)`,
   * one value per plotted fix. Aligned to the window slice, so the value for an
   * absolute track index `idx` is `ground[idx - window.takeoffIdx]`. `null`
   * until the DEM tiles resolve, or when the archive can't cover the flight.
   */
  ground: Float32Array | null;
  loading: boolean;
}

/**
 * Sample terrain ground level under the flight from the shared DEM archive: one
 * elevation per fix the altitude chart plots (1:1 with its x points, no
 * downsampling). The archive is shared with the map, so this reuses tiles the
 * map already fetched. Returns metres; consumers convert to the display unit.
 */
export const useGroundElevations = (
  track: Track | null,
  window: TrackWindow | undefined,
): GroundElevations => {
  const [ground, setGround] = useState<Float32Array | null>(null);
  const [loading, setLoading] = useState(false);

  useAsyncEffect(
    async (signal) => {
      setGround(null);
      if (!track || !window) {
        setLoading(false);
        return;
      }

      const fromIdx = window.takeoffIdx;
      const toIdx = window.landingIdx + 1;

      setLoading(true);
      try {
        const points: LngLat[] = new Array(toIdx - fromIdx);
        for (let idx = fromIdx; idx < toIdx; idx++) {
          points[idx - fromIdx] = {
            lat: track.lat[idx] / E5_PER_DEGREE,
            lng: track.lng[idx] / E5_PER_DEGREE,
          };
        }

        const elevations = await createDemLoader().elevationsAt(points, signal);
        if (!signal.aborted) {
          setGround(elevations);
        }
      } finally {
        if (!signal.aborted) {
          setLoading(false);
        }
      }
    },
    [track, window],
  );

  return { ground, loading };
};
