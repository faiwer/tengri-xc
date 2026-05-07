import { describe, expect, it } from 'vitest';
import type { Track } from './types';
import { altitudeRange } from './altitudeRange';

interface TrackInput {
  /** Per-fix altitude in metres (GPS source). */
  altMetres: number[];
  /** Optional per-fix barometric altitude in metres. */
  baroMetres?: number[];
}

const buildTrack = ({ altMetres, baroMetres }: TrackInput): Track => ({
  startTime: 0,
  t: new Uint32Array(altMetres.map((_, i) => i)),
  lat: new Int32Array(altMetres.length),
  lng: new Int32Array(altMetres.length),
  alt: new Int32Array(altMetres.map((m) => Math.round(m * 10))),
  baroAlt: baroMetres
    ? new Int32Array(baroMetres.map((m) => Math.round(m * 10)))
    : null,
});

describe('altitudeRange', () => {
  it('returns zero range for an empty interval', () => {
    const track = buildTrack({ altMetres: [1000, 1100, 1200] });
    expect(altitudeRange(track, 1, 1)).toEqual({ minAlt: 0, maxAlt: 0 });
    expect(altitudeRange(track, 2, 1)).toEqual({ minAlt: 0, maxAlt: 0 });
  });

  it('finds the highest and lowest GPS altitudes', () => {
    const track = buildTrack({
      altMetres: [1000, 1850, 1432, 2107, 1500, 980],
    });
    expect(altitudeRange(track, 0, 6)).toEqual({ minAlt: 980, maxAlt: 2107 });
  });

  it('reads GPS altitude even when baro is present', () => {
    const track = buildTrack({
      altMetres: [1000, 2000, 1500],
      baroMetres: [1500, 2500, 2000],
    });
    expect(altitudeRange(track, 0, 3)).toEqual({ minAlt: 1000, maxAlt: 2000 });
  });

  it('works without a barometer', () => {
    const track = buildTrack({
      altMetres: [1000, 1500, 800, 1200],
      // baroMetres omitted — track.baroAlt = null. The helper doesn't
      // care either way; this just exercises the no-baro branch.
    });
    expect(altitudeRange(track, 0, 4)).toEqual({ minAlt: 800, maxAlt: 1500 });
  });

  it('respects the from/to range bounds', () => {
    // Highest is at idx 0 (2500 m), lowest at idx 5 (300 m). Restricting to
    // [2, 5) should give a different pair entirely.
    const track = buildTrack({
      altMetres: [2500, 2000, 1500, 1200, 1100, 300],
    });
    expect(altitudeRange(track, 2, 5)).toEqual({ minAlt: 1100, maxAlt: 1500 });
  });

  it('rounds metres to whole numbers', () => {
    // 1234.7 m → 1235, 999.4 m → 999.
    const track = buildTrack({ altMetres: [999.4, 1234.7] });
    expect(altitudeRange(track, 0, 2)).toEqual({ minAlt: 999, maxAlt: 1235 });
  });

  it('handles a single-element range', () => {
    const track = buildTrack({ altMetres: [1500] });
    expect(altitudeRange(track, 0, 1)).toEqual({ minAlt: 1500, maxAlt: 1500 });
  });
});
