import { describe, expect, it } from 'vitest';
import { haversineM } from './haversine';

const e5 = (deg: number): number => Math.round(deg * 1e5);

describe('haversineM', () => {
  it('returns exactly zero for two coincident points', () => {
    const d = haversineM(e5(47.3769), e5(8.5417), e5(47.3769), e5(8.5417));
    expect(d).toBe(0);
  });

  it('matches the canonical Geneva → Zurich distance (~224 km, ±1 km)', () => {
    const d = haversineM(e5(46.2044), e5(6.1432), e5(47.3769), e5(8.5417));
    const km = d / 1000;
    expect(km).toBeGreaterThan(223);
    expect(km).toBeLessThan(225);
  });

  it('is symmetric to floating-point exactness', () => {
    const ab = haversineM(e5(47.0), e5(8.0), e5(47.5), e5(9.0));
    const ba = haversineM(e5(47.5), e5(9.0), e5(47.0), e5(8.0));
    expect(ab).toBe(ba);
  });

  it('one degree of latitude on the meridian is ~111.2 km', () => {
    const d = haversineM(e5(0.0), e5(0.0), e5(1.0), e5(0.0));
    const km = d / 1000;
    expect(km).toBeGreaterThan(111.0);
    expect(km).toBeLessThan(111.4);
  });

  it('one degree of longitude shrinks with latitude (cos² weighting)', () => {
    const atEquator = haversineM(e5(0.0), e5(0.0), e5(0.0), e5(1.0));
    const at47 = haversineM(e5(47.0), e5(0.0), e5(47.0), e5(1.0));
    const nearPole = haversineM(e5(89.0), e5(0.0), e5(89.0), e5(1.0));
    expect(atEquator).toBeGreaterThan(at47);
    expect(at47).toBeGreaterThan(nearPole);
    expect(nearPole).toBeLessThan(2_000);
  });
});
