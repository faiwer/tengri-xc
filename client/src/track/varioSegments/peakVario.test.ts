import { describe, expect, it } from 'vitest';
import { peakVario } from './peakVario';

describe('peakVario', () => {
  it('returns zero peaks for an empty range', () => {
    const vario = new Float32Array([1, 2, 3]);
    expect(peakVario(vario, 0, 0)).toEqual({ peakClimb: 0, peakSink: 0 });
    expect(peakVario(vario, 1, 1)).toEqual({ peakClimb: 0, peakSink: 0 });
  });

  it('finds the maximum and minimum values', () => {
    const vario = new Float32Array([0, 0.5, 2.7, 1.0, -1.4, -3.1, 0.0]);
    const { peakClimb, peakSink } = peakVario(vario, 0, vario.length);
    expect(peakClimb).toBeCloseTo(2.7, 5);
    expect(peakSink).toBeCloseTo(-3.1, 5);
  });

  it('reports zero on the side that never appears', () => {
    const climbOnly = new Float32Array([0, 1, 2, 3]);
    expect(peakVario(climbOnly, 0, climbOnly.length)).toMatchObject({
      peakClimb: 3,
      peakSink: 0,
    });
    const sinkOnly = new Float32Array([0, -1, -2, -3]);
    expect(peakVario(sinkOnly, 0, sinkOnly.length)).toMatchObject({
      peakClimb: 0,
      peakSink: -3,
    });
  });

  it('respects the from/to range bounds', () => {
    // Strongest climb (5) is at idx 0, strongest sink (-5) at idx 6. Restrict
    // to indices 2..5 — neither extreme should be visible.
    const vario = new Float32Array([5, 4, 3, 2, 1, 0, -5]);
    const { peakClimb, peakSink } = peakVario(vario, 2, 6);
    expect(peakClimb).toBe(3);
    expect(peakSink).toBe(0);
  });

  it('handles a single-element range', () => {
    const vario = new Float32Array([2.5]);
    expect(peakVario(vario, 0, 1)).toEqual({ peakClimb: 2.5, peakSink: 0 });
  });
});
