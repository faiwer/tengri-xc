import { describe, expect, it } from 'vitest';
import { computeGroundSpeed } from './groundSpeed';
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

describe('computeGroundSpeed', () => {
  it('returns an empty array for a 0-fix track', () => {
    const track = buildTrack({ t: [], latDeg: [], lonDeg: [] });
    expect(computeGroundSpeed(track)).toHaveLength(0);
  });

  it('returns 0 km/h on a stationary track', () => {
    const t: number[] = [];
    const latDeg: number[] = [];
    const lonDeg: number[] = [];
    for (let i = 0; i < 121; i++) {
      t.push(1_700_000_000 + i);
      latDeg.push(47.0);
      lonDeg.push(8.0);
    }
    const track = buildTrack({ t, latDeg, lonDeg });

    const speed = computeGroundSpeed(track);
    for (const v of speed) {
      expect(v).toBe(0);
    }
  });

  it('reports a steady ~60 km/h on a 1 Hz meridian-crossing track', () => {
    // 121 fixes is comfortably longer than the ±30 s window so the
    // central samples have a full 60 s window to displace across.
    const stepDeg = 60_000 / 3_600 / 111_195;
    const t: number[] = [];
    const latDeg: number[] = [];
    const lonDeg: number[] = [];
    for (let i = 0; i < 121; i++) {
      t.push(1_700_000_000 + i);
      latDeg.push(47.0 + i * stepDeg);
      lonDeg.push(8.0);
    }
    const track = buildTrack({ t, latDeg, lonDeg });

    const speed = computeGroundSpeed(track);
    for (let i = 30; i < speed.length - 30; i++) {
      expect(speed[i]).toBeGreaterThan(59.5);
      expect(speed[i]).toBeLessThan(60.5);
    }
  });

  it('collapses to wind drift on a closed circle in still air', () => {
    // 50 km/h airspeed on a perfect 30 s circle in still air, repeated
    // across 121 fixes (4 full loops). Inside the central window where
    // the ±30 s span covers a complete loop, displacement → 0, so
    // ground speed → 0. This is the whole reason the chart uses
    // displacement instead of path-length: it shows actual cross-
    // country progress, not airspeed-on-a-circle.
    const radiusM = ((50_000 / 3_600) * 30) / (2 * Math.PI);
    const lat0 = 47.0;
    const lon0 = 8.0;
    const oneMeterLat = 1 / 111_195;
    const oneMeterLon = 1 / (111_195 * Math.cos((lat0 * Math.PI) / 180));
    const t: number[] = [];
    const latDeg: number[] = [];
    const lonDeg: number[] = [];
    for (let i = 0; i < 121; i++) {
      const phase = (i / 30) * 2 * Math.PI;
      t.push(1_700_000_000 + i);
      latDeg.push(lat0 + radiusM * oneMeterLat * Math.sin(phase));
      lonDeg.push(lon0 + radiusM * oneMeterLon * (1 - Math.cos(phase)));
    }
    const track = buildTrack({ t, latDeg, lonDeg });

    const speed = computeGroundSpeed(track);
    // Skip edges where the ±30 s window is asymmetric and sees less
    // than one full loop. Centre samples should read essentially zero
    // (well below 5 km/h, given E5 quantisation and the chord-vs-arc
    // approximation).
    for (let i = 30; i < speed.length - 30; i++) {
      expect(speed[i]).toBeLessThan(5);
    }
  });

  it('reports drift speed on a circling pilot in steady wind', () => {
    // 50 km/h airspeed on a 30 s circle, plus 20 km/h easterly wind.
    // Net: pilot's centre drifts east at 20 km/h, so ground speed
    // should read ≈20 km/h once the window covers a full circle.
    const radiusM = ((50_000 / 3_600) * 30) / (2 * Math.PI);
    const driftMs = 20_000 / 3_600;
    const lat0 = 47.0;
    const lon0 = 8.0;
    const oneMeterLat = 1 / 111_195;
    const oneMeterLon = 1 / (111_195 * Math.cos((lat0 * Math.PI) / 180));
    const t: number[] = [];
    const latDeg: number[] = [];
    const lonDeg: number[] = [];
    for (let i = 0; i < 121; i++) {
      const phase = (i / 30) * 2 * Math.PI;
      t.push(1_700_000_000 + i);
      latDeg.push(lat0 + radiusM * oneMeterLat * Math.sin(phase));
      const driftLonDeg = i * driftMs * oneMeterLon;
      lonDeg.push(
        lon0 + radiusM * oneMeterLon * (1 - Math.cos(phase)) + driftLonDeg,
      );
    }
    const track = buildTrack({ t, latDeg, lonDeg });

    const speed = computeGroundSpeed(track);
    for (let i = 30; i < speed.length - 30; i++) {
      expect(speed[i]).toBeGreaterThan(18);
      expect(speed[i]).toBeLessThan(22);
    }
  });

  it('keeps the array aligned 1:1 with the source track', () => {
    const t: number[] = [];
    const latDeg: number[] = [];
    const lonDeg: number[] = [];
    for (let i = 0; i < 50; i++) {
      t.push(1_700_000_000 + i);
      latDeg.push(47.0 + i * 1e-4);
      lonDeg.push(8.0);
    }
    const track = buildTrack({ t, latDeg, lonDeg });

    const speed = computeGroundSpeed(track);
    expect(speed).toHaveLength(t.length);
    // No NaN or Infinity leaking through asymmetric edge windows.
    for (const v of speed) {
      expect(Number.isFinite(v)).toBe(true);
    }
  });
});
