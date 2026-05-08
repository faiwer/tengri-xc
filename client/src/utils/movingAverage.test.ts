import { describe, expect, it } from 'vitest';
import { movingAverage } from './movingAverage';

describe('movingAverage', () => {
  it('returns an empty array for empty input', () => {
    const out = movingAverage(new Uint32Array(0), new Float32Array(0), 5);
    expect(out).toHaveLength(0);
  });

  it('preserves a constant signal', () => {
    const t = new Uint32Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    const v = new Float32Array(10).fill(42);
    const out = movingAverage(t, v, 3);
    for (const x of out) {
      expect(x).toBeCloseTo(42, 5);
    }
  });

  it('cancels a symmetric oscillation around the trend', () => {
    // A 4 s period square wave at ±10 around a mean of 50, sampled at
    // 1 Hz for 200 s. A ±30 s window covers 15 full periods, so each
    // output sample averages an even number of highs and lows and lands
    // on the mean.
    const n = 201;
    const t = new Uint32Array(n);
    const v = new Float32Array(n);
    for (let i = 0; i < n; i++) {
      t[i] = i;
      v[i] = i % 4 < 2 ? 60 : 40;
    }
    const out = movingAverage(t, v, 30);
    // Skip the first/last 30 s where the window is asymmetric.
    for (let i = 30; i < n - 30; i++) {
      expect(out[i]).toBeCloseTo(50, 0);
    }
  });

  it('matches a manual centred mean on a small case', () => {
    // ±2 s window on consecutive integers — each output is the mean
    // of up to 5 neighbours.
    const t = new Uint32Array([0, 1, 2, 3, 4, 5, 6, 7]);
    const v = new Float32Array([0, 1, 2, 3, 4, 5, 6, 7]);
    const out = movingAverage(t, v, 2);
    // i=0: window [0,2] → (0+1+2)/3 = 1
    // i=1: window [0,3] → (0+1+2+3)/4 = 1.5
    // i=2: window [0,4] → (0+1+2+3+4)/5 = 2
    // i=3: window [1,5] → (1+2+3+4+5)/5 = 3
    // i=4: window [2,6] → (2+3+4+5+6)/5 = 4
    // i=5: window [3,7] → (3+4+5+6+7)/5 = 5
    // i=6: window [4,7] → (4+5+6+7)/4 = 5.5
    // i=7: window [5,7] → (5+6+7)/3 = 6
    expect(Array.from(out)).toEqual([1, 1.5, 2, 3, 4, 5, 5.5, 6]);
  });

  it('handles irregular sampling by time, not by index', () => {
    // Three samples 100 s apart — a ±30 s window contains exactly the
    // centre fix at every step, so the output equals the input.
    const t = new Uint32Array([0, 100, 200]);
    const v = new Float32Array([10, 20, 30]);
    const out = movingAverage(t, v, 30);
    expect(Array.from(out)).toEqual([10, 20, 30]);
  });
});
