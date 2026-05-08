import { describe, expect, it } from 'vitest';
import { bucketMean } from './bucketMean';

describe('bucketMean', () => {
  it('returns a copy when the input is shorter than the target', () => {
    const xs = new Uint32Array([100, 101, 102]);
    const ys = new Float32Array([10, 20, 30]);
    const out = bucketMean(xs, ys, 1500);
    expect(Array.from(out.xs)).toEqual([100, 101, 102]);
    expect(Array.from(out.ys)).toEqual([10, 20, 30]);
  });

  it('returns a copy when the input matches the target exactly', () => {
    const xs = new Uint32Array([100, 101, 102]);
    const ys = new Float32Array([10, 20, 30]);
    const out = bucketMean(xs, ys, 3);
    expect(Array.from(out.xs)).toEqual([100, 101, 102]);
    expect(Array.from(out.ys)).toEqual([10, 20, 30]);
  });

  it('preserves a constant signal', () => {
    const xs = new Uint32Array(100);
    const ys = new Float32Array(100);
    for (let i = 0; i < 100; i++) {
      xs[i] = 1_700_000_000 + i;
      ys[i] = 42;
    }
    const out = bucketMean(xs, ys, 10);
    expect(out.xs.length).toBe(10);
    expect(out.ys.length).toBe(10);
    for (const v of out.ys) {
      expect(v).toBeCloseTo(42, 5);
    }
  });

  it('averages each bucket independently', () => {
    // 12 samples → 3 buckets of 4 → bucket means {3, 7, 11}.
    const xs = new Uint32Array(12);
    const ys = new Float32Array(12);
    for (let i = 0; i < 12; i++) {
      xs[i] = 1_700_000_000 + i;
      ys[i] = i * 1; // 0..11
    }
    const out = bucketMean(xs, ys, 3);
    expect(out.xs.length).toBe(3);
    expect(out.ys.length).toBe(3);
    expect(out.ys[0]).toBeCloseTo(1.5, 5); // mean(0,1,2,3)
    expect(out.ys[1]).toBeCloseTo(5.5, 5); // mean(4,5,6,7)
    expect(out.ys[2]).toBeCloseTo(9.5, 5); // mean(8,9,10,11)
  });

  it('places the bucket centroid at the mean of bucket xs', () => {
    const xs = new Uint32Array(12);
    const ys = new Float32Array(12);
    for (let i = 0; i < 12; i++) {
      xs[i] = 1_700_000_000 + i * 10;
      ys[i] = 0;
    }
    const out = bucketMean(xs, ys, 3);
    // Bucket 0 spans xs indices [0..4), values 1.7e9 + {0,10,20,30}
    // → mean 1.7e9 + 15.
    expect(out.xs[0]).toBeCloseTo(1_700_000_015, 5);
    expect(out.xs[1]).toBeCloseTo(1_700_000_055, 5);
    expect(out.xs[2]).toBeCloseTo(1_700_000_095, 5);
  });

  it('cancels per-circle peaks and troughs into a stable trend', () => {
    // Synthesise the kind of signal the speed pipeline produces inside
    // a thermal: a sinusoidal oscillation between 5 and 95 km/h with
    // a 25 s period, around a 50 km/h mean. Bucketing to ~1500 over
    // 5000 samples (≈4 s buckets) should land each bucket on roughly
    // the 50 km/h mean, with the per-circle peaks and troughs gone.
    const xs = new Uint32Array(5000);
    const ys = new Float32Array(5000);
    for (let i = 0; i < 5000; i++) {
      xs[i] = 1_700_000_000 + i;
      ys[i] = 50 + 45 * Math.sin((i / 25) * 2 * Math.PI);
    }
    const out = bucketMean(xs, ys, 1500);

    // Inspect mid-array buckets where the proportional split has
    // settled into a clean ~3.3-sample bucket size; allow 5 km/h
    // tolerance for the residual sub-bucket phase wobble.
    for (let b = 100; b < 1400; b++) {
      expect(out.ys[b]).toBeGreaterThan(45);
      expect(out.ys[b]).toBeLessThan(55);
    }
  });
});
