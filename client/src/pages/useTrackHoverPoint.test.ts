import { describe, expect, it } from 'vitest';
import type { Track } from '../track';
import type { TrackWindow } from '../track/toPaths';
import { decimalDegree } from '../utils/geo/coordinates';
import { buildSpatialIndex, nearestTrackIndex } from './trackHoverSpatialIndex';

const latLng = (lat: number, lng: number) => ({
  lat: decimalDegree(lat),
  lng: decimalDegree(lng),
});

const bounds = (south: number, west: number, north: number, east: number) => ({
  south: decimalDegree(south),
  west: decimalDegree(west),
  north: decimalDegree(north),
  east: decimalDegree(east),
});

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
    const index = buildSpatialIndex(track, window, bounds(10, 10, 20, 20));

    expect(nearestTrackIndex(track, index, latLng(0, 0))).toBe(1);
    expect(nearestTrackIndex(track, index, latLng(100, 100))).toBe(2);
  });

  it('finds the nearest point in the same grid cell', () => {
    const track = buildTrack([
      { lat: 0, lng: 0 },
      { lat: 0.0001, lng: 0.0001 },
      { lat: 0.0002, lng: 0.0002 },
    ]);
    const window: TrackWindow = { takeoffIdx: 0, landingIdx: 2 };
    const index = buildSpatialIndex(
      track,
      window,
      bounds(0, 0, 0.0002, 0.0002),
    );

    expect(nearestTrackIndex(track, index, latLng(0.00011, 0.0001))).toBe(1);
  });

  it('expands to neighbouring cells when the cursor cell is empty', () => {
    const track = buildTrack([
      { lat: 0, lng: 0 },
      { lat: 1, lng: 1 },
      { lat: 2, lng: 2 },
    ]);
    const window: TrackWindow = { takeoffIdx: 0, landingIdx: 2 };
    const index = buildSpatialIndex(track, window, bounds(0, 0, 2, 2));

    expect(nearestTrackIndex(track, index, latLng(1.48, 1.52))).toBe(1);
    expect(nearestTrackIndex(track, index, latLng(1.8, 1.9))).toBe(2);
  });
});
