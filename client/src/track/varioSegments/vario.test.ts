import { describe, expect, it } from 'vitest';
import type { Track } from '../types';
import { computeVario } from './vario';

interface TrackInput {
  /** Per-fix Unix epoch seconds. Must be monotonically non-decreasing. */
  t: number[];
  /** Per-fix altitude in metres. Converted to decimetres internally. */
  altMetres: number[];
  /** Optional per-fix barometric altitude in metres. Same scale rule. */
  baroMetres?: number[];
}

const buildTrack = ({ t, altMetres, baroMetres }: TrackInput): Track => ({
  startTime: t[0]!,
  t: new Uint32Array(t),
  lat: new Int32Array(t.length),
  lng: new Int32Array(t.length),
  alt: new Int32Array(altMetres.map((m) => Math.round(m * 10))),
  baroAlt: baroMetres
    ? new Int32Array(baroMetres.map((m) => Math.round(m * 10)))
    : null,
});

describe('computeVario', () => {
  it('reports zero on a constant-altitude track', () => {
    const track = buildTrack({
      t: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
      altMetres: Array.from({ length: 13 }, () => 1000),
    });
    const vario = computeVario(track);
    for (const v of vario) {
      expect(v).toBeCloseTo(0, 6);
    }
  });

  it('reports the slope of a steady climb', () => {
    // 2 m/s climb over 21 seconds — well clear of the ±5 s window so the
    // centre samples should read exactly +2 m/s.
    const t = Array.from({ length: 21 }, (_, i) => i);
    const altMetres = t.map((sec) => 1000 + sec * 2);
    const vario = computeVario(buildTrack({ t, altMetres }));
    for (let i = 5; i < 16; i++) {
      expect(vario[i]).toBeCloseTo(2, 4);
    }
  });

  it('reports a steady sink as a negative slope', () => {
    const t = Array.from({ length: 21 }, (_, i) => i);
    const altMetres = t.map((sec) => 1000 - sec * 1.5);
    const vario = computeVario(buildTrack({ t, altMetres }));
    for (let i = 5; i < 16; i++) {
      expect(vario[i]).toBeCloseTo(-1.5, 4);
    }
  });

  it('uses barometric altitude when present, ignoring GPS alt', () => {
    const t = Array.from({ length: 21 }, (_, i) => i);
    const altMetres = t.map(() => 1000);
    const baroMetres = t.map((sec) => 500 + sec * 3);
    const vario = computeVario(buildTrack({ t, altMetres, baroMetres }));
    for (let i = 5; i < 16; i++) {
      expect(vario[i]).toBeCloseTo(3, 4);
    }
  });

  it('falls back to GPS altitude when baroAlt is null', () => {
    const t = Array.from({ length: 21 }, (_, i) => i);
    const altMetres = t.map((sec) => 1000 + sec * 4);
    const vario = computeVario(buildTrack({ t, altMetres }));
    for (let i = 5; i < 16; i++) {
      expect(vario[i]).toBeCloseTo(4, 4);
    }
  });

  it('handles array boundaries with a one-sided window', () => {
    // Steady +2 m/s climb. The ±5 s window collapses near the edges, so
    // boundary samples are computed over a partial window — they should
    // still be ~+2 m/s, not zero or NaN.
    const t = Array.from({ length: 21 }, (_, i) => i);
    const altMetres = t.map((sec) => 1000 + sec * 2);
    const vario = computeVario(buildTrack({ t, altMetres }));
    expect(vario[0]).toBeCloseTo(2, 4);
    expect(vario[vario.length - 1]).toBeCloseTo(2, 4);
  });

  it('survives a track that is shorter than the smoothing window', () => {
    const track = buildTrack({
      t: [0, 1, 2],
      altMetres: [1000, 1003, 1006],
    });
    const vario = computeVario(track);
    expect(vario.length).toBe(3);
    for (const v of vario) {
      expect(v).toBeCloseTo(3, 4);
    }
  });

  it('returns zero for a single-fix track (no time delta)', () => {
    const track = buildTrack({ t: [0], altMetres: [1000] });
    const vario = computeVario(track);
    expect(vario.length).toBe(1);
    expect(vario[0]).toBe(0);
  });
});
