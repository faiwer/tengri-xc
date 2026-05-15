import { useMemo } from 'react';
import type { TrackMetadata } from '../api/tracks.io';
import type { Track } from '../track';
import {
  buildFlightAnalysis,
  type FlightAnalysis,
} from '../track/flightAnalysis';

export function useFlightAnalysis(
  track: Track | null,
  metadata?: TrackMetadata,
): FlightAnalysis | null {
  return useMemo(
    () => (track && metadata ? buildFlightAnalysis(track, metadata) : null),
    [track, metadata],
  );
}
