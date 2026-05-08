import { describe, expect, it } from 'vitest';
import { computePathSpeed } from './pathSpeed';
import type { Track } from './types';

interface TrackInput {
  /** Per-fix Unix epoch seconds, monotonically non-decreasing. */
  t: number[];
  /** Per-fix latitude in decimal degrees. */
  latDeg: number[];
  /** Per-fix longitude in decimal degrees. */
  lonDeg: number[];
}

const buildTrack = ({ t, latDeg, lonDeg }: TrackInput): Track => ({
  startTime: t[0]!,
  t: new Uint32Array(t),
  lat: new Int32Array(latDeg.map((deg) => Math.round(deg * 1e5))),
  lng: new Int32Array(lonDeg.map((deg) => Math.round(deg * 1e5))),
  alt: new Int32Array(t.length),
  baroAlt: null,
  tas: null,
});

describe('computePathSpeed', () => {
  it('returns an empty array for a 0-fix track', () => {
    const track = buildTrack({ t: [], latDeg: [], lonDeg: [] });
    expect(computePathSpeed(track)).toHaveLength(0);
  });

  it('returns 0 km/h on a stationary 1 Hz track', () => {
    const t: number[] = [];
    const latDeg: number[] = [];
    const lonDeg: number[] = [];
    for (let i = 0; i < 11; i++) {
      t.push(1_700_000_000 + i);
      latDeg.push(47.0);
      lonDeg.push(8.0);
    }
    const track = buildTrack({ t, latDeg, lonDeg });

    const speed = computePathSpeed(track);
    for (const v of speed) {
      expect(v).toBe(0);
    }
  });

  it('reports a steady ~60 km/h on a 1 Hz meridian-crossing track', () => {
    // 21 evenly spaced 1 Hz fixes flying due north at 60 km/h.
    // 60 km/h = 16.667 m/s; 1° lat ≈ 111 195 m → 1.499 × 10⁻⁴ ° / s.
    const stepDeg = 60_000 / 3_600 / 111_195;
    const t: number[] = [];
    const latDeg: number[] = [];
    const lonDeg: number[] = [];
    for (let i = 0; i < 21; i++) {
      t.push(1_700_000_000 + i);
      latDeg.push(47.0 + i * stepDeg);
      lonDeg.push(8.0);
    }
    const track = buildTrack({ t, latDeg, lonDeg });

    const speed = computePathSpeed(track);
    // Allow ±0.5 km/h for E5 quantisation noise.
    for (const v of speed) {
      expect(v).toBeGreaterThan(59.5);
      expect(v).toBeLessThan(60.5);
    }
  });

  it('reports airspeed (not wind drift) on a closed circle in still air', () => {
    // 50 km/h airspeed on a perfect 30 s circle in still air. Per-leg
    // chord ≈ airspeed × Δt, so per-fix path speed reads ≈ 50 km/h
    // throughout, never collapsing to zero. This is the property
    // that makes path speed a valid airspeed proxy in thermalling.
    const radiusM = ((50_000 / 3_600) * 30) / (2 * Math.PI);
    const lat0 = 47.0;
    const lon0 = 8.0;
    const oneMeterLat = 1 / 111_195;
    const oneMeterLon = 1 / (111_195 * Math.cos((lat0 * Math.PI) / 180));
    const t: number[] = [];
    const latDeg: number[] = [];
    const lonDeg: number[] = [];
    for (let i = 0; i <= 60; i++) {
      const phase = (i / 30) * 2 * Math.PI;
      t.push(1_700_000_000 + i);
      latDeg.push(lat0 + radiusM * oneMeterLat * Math.sin(phase));
      lonDeg.push(lon0 + radiusM * oneMeterLon * (1 - Math.cos(phase)));
    }
    const track = buildTrack({ t, latDeg, lonDeg });

    const speed = computePathSpeed(track);
    // Two competing effects on each per-leg reading:
    // - Chord underestimate: a chord is shorter than its arc (~12°
    //   leg → chord/arc ≈ 0.995), pulling readings ~0.5% below 50.
    // - E5 quantisation: rounding the lat/lon to micro-degrees
    //   inflates per-leg distances by up to ~1 m (~3.6 km/h at 1 Hz),
    //   pushing readings above 50.
    // The two roughly cancel on average; the bounds give the noise
    // floor enough room to be honest about both.
    for (let i = 1; i < speed.length; i++) {
      expect(speed[i]).toBeGreaterThan(46);
      expect(speed[i]).toBeLessThan(54);
    }
  });
});
