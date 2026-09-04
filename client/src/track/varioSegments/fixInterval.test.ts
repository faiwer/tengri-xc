import { describe, expect, it } from 'vitest';
import type { Track } from '../types';
import { averageFixInterval } from './fixInterval';

const buildTrack = (t: number[]): Track => ({
  startTime: t[0]!,
  t: new Uint32Array(t),
  lat: new Int32Array(t.length),
  lng: new Int32Array(t.length),
  alt: new Int32Array(t.length),
  baroAlt: null,
  tas: null,
});

describe('averageFixInterval', () => {
  it('averages over the flight window, ignoring fixes outside it', () => {
    const track = buildTrack([0, 500, 501, 502, 503, 900]);
    const interval = averageFixInterval(track, {
      takeoffIdx: 1,
      landingIdx: 4,
    });
    expect(interval).toBe(1);
  });

  it('smears a single long gap across the whole window', () => {
    const track = buildTrack([0, 1, 2, 33]);
    const interval = averageFixInterval(track, {
      takeoffIdx: 0,
      landingIdx: 3,
    });
    expect(interval).toBe(11);
  });

  it('returns zero for a window holding a single fix', () => {
    const track = buildTrack([0, 1, 2]);
    const interval = averageFixInterval(track, {
      takeoffIdx: 1,
      landingIdx: 1,
    });
    expect(interval).toBe(0);
  });
});
