import { describe, expect, it } from 'vitest';
import { buildVarioSegments, type VarioSegment } from './segments';

interface BucketRun {
  bucket: number;
  /** Run length in seconds (== fix count, since fixtures are 1 Hz). */
  durationSeconds: number;
}

/**
 * Build aligned `(buckets, times)` arrays from a list of bucket runs at
 * 1 Hz. Times are monotonically increasing Unix-epoch seconds starting at 0.
 */
const buildBuckets = (
  runs: BucketRun[],
): { buckets: Int8Array; times: Uint32Array } => {
  const totalFixes = runs.reduce((acc, run) => acc + run.durationSeconds, 0);
  const buckets = new Int8Array(totalFixes);
  const times = new Uint32Array(totalFixes);
  let writeIdx = 0;
  for (const run of runs) {
    for (let i = 0; i < run.durationSeconds; i++) {
      buckets[writeIdx] = run.bucket;
      times[writeIdx] = writeIdx;
      writeIdx++;
    }
  }
  return { buckets, times };
};

const segmentBuckets = (segments: VarioSegment[]): number[] =>
  segments.map((s) => s.bucket);

describe('buildVarioSegments', () => {
  it('returns an empty array when the range is empty', () => {
    const { buckets, times } = buildBuckets([
      { bucket: 0, durationSeconds: 30 },
    ]);
    expect(buildVarioSegments(buckets, times, 0, 0)).toEqual([]);
    expect(buildVarioSegments(buckets, times, 5, 5)).toEqual([]);
  });

  it('emits a single segment for a single-bucket track', () => {
    const { buckets, times } = buildBuckets([
      { bucket: 2, durationSeconds: 60 },
    ]);
    const segments = buildVarioSegments(buckets, times, 0, 60);
    expect(segments).toHaveLength(1);
    expect(segments[0]).toMatchObject({ bucket: 2, startIdx: 0, endIdx: 60 });
  });

  it('coalesces consecutive runs of the same bucket', () => {
    const { buckets, times } = buildBuckets([
      { bucket: 2, durationSeconds: 30 },
      // No bucket switch between two adjacent identical runs — buildBuckets
      // produces them as one continuous span; the test still covers that
      // coalesce keeps it as one segment.
      { bucket: 2, durationSeconds: 30 },
    ]);
    const segments = buildVarioSegments(buckets, times, 0, 60);
    expect(segments).toHaveLength(1);
    expect(segments[0]).toMatchObject({ bucket: 2, startIdx: 0, endIdx: 60 });
  });

  it('reabsorbs an interior run shorter than the 8 s threshold', () => {
    const { buckets, times } = buildBuckets([
      { bucket: 2, durationSeconds: 60 },
      { bucket: -1, durationSeconds: 5 },
      { bucket: 2, durationSeconds: 60 },
    ]);
    const segments = buildVarioSegments(buckets, times, 0, 125);
    expect(segments).toHaveLength(1);
    expect(segments[0]).toMatchObject({ bucket: 2, startIdx: 0, endIdx: 125 });
  });

  it('keeps an interior run that exceeds the 8 s threshold', () => {
    const { buckets, times } = buildBuckets([
      { bucket: 2, durationSeconds: 60 },
      { bucket: -1, durationSeconds: 30 },
      { bucket: 2, durationSeconds: 60 },
    ]);
    const segments = buildVarioSegments(buckets, times, 0, 150);
    expect(segmentBuckets(segments)).toEqual([2, -1, 2]);
  });

  it('does not reabsorb across different-bucket flanks', () => {
    // [+2, -1(short), -3] — the short -1 is *not* sandwiched between two
    // same-bucket runs, so reabsorb does nothing. The dissolveShortRuns
    // pass then merges the -1 into the larger neighbour (the -3).
    const { buckets, times } = buildBuckets([
      { bucket: 2, durationSeconds: 60 },
      { bucket: -1, durationSeconds: 5 },
      { bucket: -3, durationSeconds: 60 },
    ]);
    const segments = buildVarioSegments(buckets, times, 0, 125);
    expect(segmentBuckets(segments)).toEqual([2, -3]);
  });

  it('dissolves a sub-15 s run into its larger neighbour', () => {
    // [+2(60s), -3(10s), +1(120s)] — the -3 is < 15 s and isn't sandwiched
    // by same-bucket flanks, so it dissolves into the larger neighbour
    // (the +1, 120 s).
    const { buckets, times } = buildBuckets([
      { bucket: 2, durationSeconds: 60 },
      { bucket: -3, durationSeconds: 10 },
      { bucket: 1, durationSeconds: 120 },
    ]);
    const segments = buildVarioSegments(buckets, times, 0, 190);
    expect(segmentBuckets(segments)).toEqual([2, 1]);
    // The merged +1 absorbs the -3's points, so it spans 70..190.
    expect(segments[1]).toMatchObject({ bucket: 1, endIdx: 190 });
  });

  it('keeps a run that meets the 15 s threshold exactly', () => {
    // 15 s is the cutoff — `>= 15` survives.
    const { buckets, times } = buildBuckets([
      { bucket: 2, durationSeconds: 60 },
      { bucket: -3, durationSeconds: 16 }, // 16 s span = 15 s duration
      { bucket: 1, durationSeconds: 60 },
    ]);
    const segments = buildVarioSegments(buckets, times, 0, 136);
    expect(segmentBuckets(segments)).toEqual([2, -3, 1]);
  });

  it('iterates dissolution until no short runs remain', () => {
    // Cascade: every run is 10 s. After the first dissolve, the merged run
    // is 20 s and survives, but neighbouring shorts still need to dissolve.
    const { buckets, times } = buildBuckets([
      { bucket: 2, durationSeconds: 10 },
      { bucket: -1, durationSeconds: 10 },
      { bucket: 3, durationSeconds: 10 },
      { bucket: -2, durationSeconds: 10 },
    ]);
    const segments = buildVarioSegments(buckets, times, 0, 40);
    // Exact result depends on tie-break order, but the invariant is: every
    // surviving segment must hit the threshold or be the only one left.
    if (segments.length > 1) {
      for (const segment of segments) {
        const span = times[segment.endIdx - 1]! - times[segment.startIdx]!;
        expect(span).toBeGreaterThanOrEqual(15);
      }
    } else {
      expect(segments).toHaveLength(1);
    }
  });

  it('respects the from/to range bounds', () => {
    // Build a track that spans far longer than the segmentation window;
    // the result should reflect only the requested slice.
    const { buckets, times } = buildBuckets([
      { bucket: 2, durationSeconds: 30 },
      { bucket: -3, durationSeconds: 60 },
      { bucket: 1, durationSeconds: 30 },
    ]);
    const segments = buildVarioSegments(buckets, times, 30, 90);
    expect(segments).toHaveLength(1);
    expect(segments[0]).toMatchObject({ bucket: -3, startIdx: 30, endIdx: 90 });
  });
});
