import { describe, expect, it } from 'vitest';
import { classifyBuckets, MAX_BUCKET, MIN_BUCKET } from './classify';

describe('classifyBuckets', () => {
  it('floors fractional vario values into integer buckets', () => {
    const vario = new Float32Array([0, 0.3, 0.999, 1, 1.5, 2.7]);
    const buckets = classifyBuckets(vario);
    expect(Array.from(buckets)).toEqual([0, 0, 0, 1, 1, 2]);
  });

  it('floors negative values toward minus infinity', () => {
    const vario = new Float32Array([-0.001, -0.5, -1, -1.999, -2]);
    const buckets = classifyBuckets(vario);
    expect(Array.from(buckets)).toEqual([-1, -1, -1, -2, -2]);
  });

  it(`clamps anything below MIN_BUCKET to ${MIN_BUCKET}`, () => {
    const vario = new Float32Array([-5, -5.001, -10, -100]);
    const buckets = classifyBuckets(vario);
    expect(Array.from(buckets)).toEqual([-5, -5, -5, -5]);
  });

  it(`clamps anything at or above MAX_BUCKET to ${MAX_BUCKET}`, () => {
    const vario = new Float32Array([5, 5.5, 10, 100]);
    const buckets = classifyBuckets(vario);
    expect(Array.from(buckets)).toEqual([5, 5, 5, 5]);
  });

  it('returns an empty result for an empty input', () => {
    const buckets = classifyBuckets(new Float32Array(0));
    expect(buckets.length).toBe(0);
  });
});
