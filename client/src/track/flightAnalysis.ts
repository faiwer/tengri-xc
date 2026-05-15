import type { TrackMetadata } from '../api/tracks.io';
import { altitudeRange, type AltitudeRange } from './altitudeRange';
import { findIndexAt } from './findIndexAt';
import { computeGroundSpeed } from './groundSpeed';
import { computePathSpeed } from './pathSpeed';
import { smoothTas, smoothTrackSpeed } from './smoothedSpeed';
import type { Track } from './types';
import { trackToPaths, type TrackPath, type TrackWindow } from './toPaths';
import { classifyBuckets } from './varioSegments/classify';
import { peakVario, type VarioPeaks } from './varioSegments/peakVario';
import { buildVarioSegments } from './varioSegments/segments';
import { computeVario } from './varioSegments/vario';

export interface FlightAnalysis {
  /** Decoded full-resolution track used by every derived field below. */
  track: Track;
  /** Whole-second offset from UTC used to display flight-local wall clock. */
  timeOffsetSeconds: number;
  /** Flight slice selected from metadata takeoff/landing timestamps. */
  window: TrackWindow;
  /** Per-fix series shared by charts, map colouring, and cursor readouts. */
  metrics: FlightMetrics;
  /** Vario-derived peaks and colour segments over the flight window. */
  vario: FlightVarioAnalysis;
  /** GPS altitude min/max over the flight window, in metres. */
  altitudes: AltitudeRange;
  /** Map polylines for pre-flight, flight, and post-flight runs. */
  paths: TrackPath[];
  /** Bounding box of the flight window, used for map fit and hover indexing. */
  bounds: google.maps.LatLngBoundsLiteral | null;
}

export interface FlightMetrics {
  /**
   * Per-fix ground speed in km/h. Uses straight-line displacement across a
   * centered +/-30 s window, so turning noise is smoothed out.
   */
  speed: Float32Array;
  /** Per-fix path speed in km/h, smoothed to match the speed chart. */
  pathSpeed: Float32Array;
  /** Per-fix TAS in km/h, smoothed to match the speed chart when present. */
  tas: Float32Array | null;
  /**
   * Per-fix vertical velocity in m/s. Uses centered +/-5 s altitude slope,
   * preferring baro altitude when present and GPS altitude otherwise.
   */
  vario: Float32Array;
}

export interface FlightVarioAnalysis extends VarioPeaks {
  /**
   * Merged vario bucket runs over the flight window. Buckets come from the
   * smoothed {@link FlightMetrics.vario} series, then short noisy runs are
   * absorbed before map colouring.
   */
  segments: ReturnType<typeof buildVarioSegments>;
}

export const buildFlightAnalysis = (
  track: Track,
  metadata: Pick<TrackMetadata, 'takeoffAt' | 'landingAt' | 'takeoffOffset'>,
): FlightAnalysis => {
  const window: TrackWindow = {
    takeoffIdx: findIndexAt(track, metadata.takeoffAt),
    landingIdx: findIndexAt(track, metadata.landingAt),
  };
  const toIdx = window.landingIdx + 1;
  const metrics: FlightMetrics = {
    speed: computeGroundSpeed(track),
    pathSpeed: smoothTrackSpeed(track.t, computePathSpeed(track)),
    tas: track.tas ? smoothTas(track.t, track.tas) : null,
    vario: computeVario(track),
  };
  const vario: FlightVarioAnalysis = buildFlightVarioAnalysis(
    track,
    metrics.vario,
    window,
  );
  const paths: TrackPath[] = trackToPaths(track, window, vario.segments);

  return {
    track,
    timeOffsetSeconds: metadata.takeoffOffset,
    window,
    metrics,
    vario,
    altitudes: altitudeRange(track, window.takeoffIdx, toIdx),
    paths,
    bounds: trackBounds(track, window),
  };
};

const buildFlightVarioAnalysis = (
  track: Track,
  vario: Float32Array,
  window: TrackWindow,
): FlightVarioAnalysis => {
  const fromIdx = window.takeoffIdx;
  const toIdx = window.landingIdx + 1;
  const buckets = classifyBuckets(vario);
  const segments = buildVarioSegments(buckets, track.t, fromIdx, toIdx);
  const { peakClimb, peakSink } = peakVario(vario, fromIdx, toIdx);

  return { segments, peakClimb, peakSink };
};

const trackBounds = (
  track: Track,
  window: TrackWindow,
): google.maps.LatLngBoundsLiteral | null => {
  const fromIdx = window.takeoffIdx;
  const toIdx = window.landingIdx + 1;
  if (toIdx <= fromIdx) {
    return null;
  }

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
};
