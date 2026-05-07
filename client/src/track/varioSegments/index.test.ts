import { describe, expect, it } from 'vitest';
import type { Track } from '../types';
import { computeVarioInsights } from './index';

interface TrackInput {
  /** Per-fix Unix epoch seconds, monotonically non-decreasing. */
  t: number[];
  /** Per-fix barometric altitude in metres. */
  baroMetres: number[];
}

const buildTrack = ({ t, baroMetres }: TrackInput): Track => ({
  startTime: t[0]!,
  t: new Uint32Array(t),
  lat: new Int32Array(t.length),
  lng: new Int32Array(t.length),
  alt: new Int32Array(t.length),
  baroAlt: new Int32Array(baroMetres.map((m) => Math.round(m * 10))),
});

interface AltLeg {
  /** Length in seconds (1 Hz fixtures). */
  seconds: number;
  /** Climb rate during this leg in m/s. */
  climbRate: number;
}

/**
 * Build a synthetic baro-alt track from a sequence of legs. Returns a
 * `Track` with 1 Hz fixes and a barometric altitude that ramps linearly
 * within each leg.
 */
const trackFromLegs = (legs: AltLeg[]): Track => {
  const t: number[] = [];
  const baroMetres: number[] = [];
  let timeSec = 0;
  let altMetres = 1000;
  for (const leg of legs) {
    for (let i = 0; i < leg.seconds; i++) {
      t.push(timeSec);
      baroMetres.push(altMetres);
      timeSec++;
      altMetres += leg.climbRate;
    }
  }
  return buildTrack({ t, baroMetres });
};

describe('computeVarioInsights', () => {
  it('splits a climb / glide / climb track into three segments', () => {
    const track = trackFromLegs([
      { seconds: 120, climbRate: 2 }, // strong climb (~+2 m/s)
      { seconds: 120, climbRate: -1 }, // glide (~-1 m/s)
      { seconds: 120, climbRate: 3 }, // strong climb (~+3 m/s)
    ]);
    const { segments } = computeVarioInsights(track, 0, track.t.length);
    expect(segments.length).toBeGreaterThanOrEqual(3);
    const buckets = segments.map((s) => s.bucket);
    expect(buckets).toContain(2);
    expect(buckets).toContain(-1);
    expect(buckets).toContain(3);
  });

  it('does not split on a brief vario excursion inside a thermal', () => {
    // Steady climb with a 5 s blip of sink in the middle. The reabsorb pass
    // should swallow the blip; the result is a single climb segment.
    const track = trackFromLegs([
      { seconds: 60, climbRate: 2 },
      { seconds: 5, climbRate: -3 },
      { seconds: 60, climbRate: 2 },
    ]);
    const { segments } = computeVarioInsights(track, 0, track.t.length);
    const dominantBucket = segments
      .slice()
      .sort((a, b) => b.endIdx - b.startIdx - (a.endIdx - a.startIdx))[0]!;
    expect(dominantBucket.bucket).toBe(2);
  });

  it('clamps strong sink to bucket -5', () => {
    const track = trackFromLegs([
      { seconds: 60, climbRate: -1 },
      { seconds: 60, climbRate: -8 }, // way past -5 m/s
      { seconds: 60, climbRate: -1 },
    ]);
    const { segments } = computeVarioInsights(track, 0, track.t.length);
    expect(segments.some((s) => s.bucket === -5)).toBe(true);
  });

  it('clamps strong climb to bucket +5', () => {
    const track = trackFromLegs([
      { seconds: 60, climbRate: 1 },
      { seconds: 60, climbRate: 9 }, // way past +5 m/s
      { seconds: 60, climbRate: 1 },
    ]);
    const { segments } = computeVarioInsights(track, 0, track.t.length);
    expect(segments.some((s) => s.bucket === 5)).toBe(true);
  });

  it('returns no segments and zero peaks when the range is empty', () => {
    const track = trackFromLegs([{ seconds: 60, climbRate: 2 }]);
    const insights = computeVarioInsights(track, 30, 30);
    expect(insights.segments).toEqual([]);
    expect(insights.peakClimb).toBe(0);
    expect(insights.peakSink).toBe(0);
  });

  it('honours the requested from/to bounds', () => {
    // Overall track has glide → climb → glide. Restrict to the glide tail.
    const track = trackFromLegs([
      { seconds: 60, climbRate: -1 },
      { seconds: 60, climbRate: 2 },
      { seconds: 60, climbRate: -1 },
    ]);
    const { segments } = computeVarioInsights(track, 120, 180);
    expect(segments.length).toBeGreaterThanOrEqual(1);
    for (const segment of segments) {
      expect(segment.startIdx).toBeGreaterThanOrEqual(120);
      expect(segment.endIdx).toBeLessThanOrEqual(180);
    }
  });

  it('reports peak climb and peak sink over the flight window', () => {
    const track = trackFromLegs([
      { seconds: 60, climbRate: 2 }, // ~+2 m/s sustained
      { seconds: 60, climbRate: -3 }, // ~-3 m/s sustained
      { seconds: 60, climbRate: 4 }, // ~+4 m/s sustained, this is the peak
    ]);
    const { peakClimb, peakSink } = computeVarioInsights(
      track,
      0,
      track.t.length,
    );
    // The smoothed signal at the centre of each leg is close to that leg's
    // slope; we check the peak is at least a clean fraction of the input.
    expect(peakClimb).toBeGreaterThan(3.5);
    expect(peakClimb).toBeLessThanOrEqual(4);
    expect(peakSink).toBeLessThan(-2.5);
    expect(peakSink).toBeGreaterThanOrEqual(-3);
  });

  it('confines peaks to the requested range', () => {
    // Peak climb is in the first leg, but we restrict to legs 2 + 3 only.
    const track = trackFromLegs([
      { seconds: 60, climbRate: 5 }, // peak climb here
      { seconds: 60, climbRate: -2 },
      { seconds: 60, climbRate: 1 },
    ]);
    const { peakClimb, peakSink } = computeVarioInsights(track, 60, 180);
    expect(peakClimb).toBeLessThan(2); // first leg's +5 m/s must NOT count
    expect(peakSink).toBeLessThan(-1); // glide leg's -2 m/s does count
  });
});
