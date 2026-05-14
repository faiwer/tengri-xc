import { describe, expect, it } from 'vitest';
import type { Track } from '../track';
import type { TrackWindow } from '../track/toPaths';
import { buildSpatialIndex, nearestTrackIndex } from './trackHoverSpatialIndex';

const buildTrack = (points: { lat: number; lng: number }[]): Track => ({
  startTime: 0,
  t: new Uint32Array(points.map((_, idx) => idx)),
  lat: new Int32Array(points.map((point) => Math.round(point.lat * 1e5))),
  lng: new Int32Array(points.map((point) => Math.round(point.lng * 1e5))),
  alt: new Int32Array(points.length),
  baroAlt: null,
  tas: null,
});

describe('track hover spatial index', () => {
  it('indexes only the flight window fixes', () => {
    const track = buildTrack([
      { lat: 0, lng: 0 },
      { lat: 10, lng: 10 },
      { lat: 20, lng: 20 },
      { lat: 30, lng: 30 },
    ]);
    const window: TrackWindow = { takeoffIdx: 1, landingIdx: 2 };
    const index = buildSpatialIndex(track, window, {
      south: 10,
      west: 10,
      north: 20,
      east: 20,
    });

    expect(nearestTrackIndex(track, index, { lat: 0, lng: 0 })).toBe(1);
    expect(nearestTrackIndex(track, index, { lat: 100, lng: 100 })).toBe(2);
  });

  it('finds the nearest point in the same grid cell', () => {
    const track = buildTrack([
      { lat: 0, lng: 0 },
      { lat: 0.0001, lng: 0.0001 },
      { lat: 0.0002, lng: 0.0002 },
    ]);
    const window: TrackWindow = { takeoffIdx: 0, landingIdx: 2 };
    const index = buildSpatialIndex(track, window, {
      south: 0,
      west: 0,
      north: 0.0002,
      east: 0.0002,
    });

    expect(nearestTrackIndex(track, index, { lat: 0.00011, lng: 0.0001 })).toBe(
      1,
    );
  });

  it('expands to neighbouring cells when the cursor cell is empty', () => {
    const track = buildTrack([
      { lat: 0, lng: 0 },
      { lat: 1, lng: 1 },
      { lat: 2, lng: 2 },
    ]);
    const window: TrackWindow = { takeoffIdx: 0, landingIdx: 2 };
    const index = buildSpatialIndex(track, window, {
      south: 0,
      west: 0,
      north: 2,
      east: 2,
    });

    expect(nearestTrackIndex(track, index, { lat: 1.48, lng: 1.52 })).toBe(1);
    expect(nearestTrackIndex(track, index, { lat: 1.8, lng: 1.9 })).toBe(2);
  });
});
