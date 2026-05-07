import { EnumVariantValue } from 'bincode-ts';
import { describe, expect, it } from 'vitest';
import type { TengriFile } from '../../api/tracks.io';
import { TrackDecodeError } from '../types';
import { decodeTrack } from './index';
import { computeCompactHash } from './hash';

const START_TIME = 1_700_000_000;
const INTERVAL = 1;

const FIXES = [
  {
    idx: 0,
    lat: 4_700_000,
    lon: 1_300_000,
    geo_alt: 10_000,
    pressure_alt: 10_100,
  },
  {
    idx: 2,
    lat: 4_700_020,
    lon: 1_299_980,
    geo_alt: 10_010,
    pressure_alt: 10_110,
  },
];

const COORDS = [
  { lat: 1, lon: -1, geo_alt: 5, pressure_alt: 4 },
  { lat: -1, lon: 1, geo_alt: 5, pressure_alt: 4 },
];

const TIME_FIXES = [{ idx: 0, time: START_TIME }];

const buildFile = (hash: number): TengriFile => ({
  version: 3,
  metadata: {},
  track: {
    start_time: START_TIME,
    interval: INTERVAL,
    track: EnumVariantValue('Dual', { fixes: FIXES, coords: COORDS }),
    time_fixes: TIME_FIXES,
    hash,
  },
});

const validHash = computeCompactHash(
  START_TIME,
  INTERVAL,
  { dual: true, fixes: FIXES, coords: COORDS },
  TIME_FIXES,
);

describe('decodeTrack', () => {
  it('reconstructs an SoA track from a tiny dual-format file', async () => {
    const track = await decodeTrack(buildFile(validHash));

    expect(track.startTime).toBe(START_TIME);

    expect(Array.from(track.t)).toEqual([
      START_TIME,
      START_TIME + 1,
      START_TIME + 2,
      START_TIME + 3,
    ]);

    // idx 0: anchor fix.
    // idx 1: anchor + first delta coord.
    // idx 2: fresh fix (absolute, not anchor + delta).
    // idx 3: idx-2 fix + second delta coord.
    expect(Array.from(track.lat)).toEqual([
      4_700_000, 4_700_001, 4_700_020, 4_700_019,
    ]);
    expect(Array.from(track.lng)).toEqual([
      1_300_000, 1_299_999, 1_299_980, 1_299_981,
    ]);
    expect(Array.from(track.alt)).toEqual([10_000, 10_005, 10_010, 10_015]);

    expect(track.baroAlt).not.toBeNull();
    expect(Array.from(track.baroAlt!)).toEqual([
      10_100, 10_104, 10_110, 10_114,
    ]);
  });

  it('throws TrackDecodeError when the embedded hash does not match', async () => {
    await expect(decodeTrack(buildFile(0xdeadbeef))).rejects.toBeInstanceOf(
      TrackDecodeError,
    );
  });
});
