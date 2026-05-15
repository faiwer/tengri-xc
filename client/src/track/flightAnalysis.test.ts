import { describe, expect, it } from 'vitest';
import type { TrackMetadata } from '../api/tracks.io';
import { buildFlightAnalysis } from './flightAnalysis';
import type { Track } from './types';

const buildTrack = (input: {
  times: number[];
  lat: number[];
  lng: number[];
  altMetres: number[];
}): Track => ({
  startTime: input.times[0] ?? 0,
  t: new Uint32Array(input.times),
  lat: new Int32Array(input.lat.map((value) => Math.round(value * 1e5))),
  lng: new Int32Array(input.lng.map((value) => Math.round(value * 1e5))),
  alt: new Int32Array(input.altMetres.map((value) => Math.round(value * 10))),
  baroAlt: null,
  tas: null,
});

const metadata = (takeoffAt: number, landingAt: number): TrackMetadata => ({
  id: 'track-1',
  pilot: { name: 'Pilot', country: null },
  takeoffAt,
  landingAt,
  takeoffOffset: 0,
  landingOffset: 0,
  takeoff: { lat: 0, lon: 0 },
  landing: { lat: 0, lon: 0 },
  compressionRatio: 1,
});

describe('buildFlightAnalysis', () => {
  it('builds shared flight analysis data', () => {
    const track = buildTrack({
      times: [0, 10, 20, 30],
      lat: [45, 46, 47, 48],
      lng: [7, 8, 9, 10],
      altMetres: [1000, 1200, 900, 1100],
    });

    const analysis = buildFlightAnalysis(track, metadata(10, 20));

    expect(analysis.track).toBe(track);
    expect(analysis.timeOffsetSeconds).toBe(0);
    expect(analysis.window).toEqual({ takeoffIdx: 1, landingIdx: 2 });
    expect(analysis.altitudes).toEqual({ minAlt: 900, maxAlt: 1200 });
    expect(analysis.bounds).toEqual({ south: 46, west: 8, north: 47, east: 9 });
    expect(analysis.metrics.speed).toHaveLength(track.t.length);
    expect(analysis.metrics.vario).toHaveLength(track.t.length);
    expect(analysis.vario.segments.length).toBeGreaterThan(0);
    expect(analysis.paths.length).toBeGreaterThan(0);
  });

  it('uses the shared vario array for peak values', () => {
    const track = buildTrack({
      times: Array.from({ length: 21 }, (_, idx) => idx),
      lat: Array.from({ length: 21 }, () => 45),
      lng: Array.from({ length: 21 }, () => 7),
      altMetres: Array.from({ length: 21 }, (_, idx) => 1000 + idx * 2),
    });

    const analysis = buildFlightAnalysis(track, metadata(5, 15));

    expect(analysis.vario.peakClimb).toBeCloseTo(2, 4);
    expect(analysis.vario.peakSink).toBe(0);
  });
});
