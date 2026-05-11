import { EnumVariantValue } from 'bincode-ts';
import { describe, expect, it } from 'vitest';
import type { TasFix, TengriFile } from '../../api/tracks.io';
import { TrackDecodeError } from '../types';
import { decodeTrack } from './index';
import { computeCompactHash } from './hash';
import type { UnpackedTas } from './unpackBody';

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

const NO_TAS_UNPACKED: UnpackedTas = { kind: 'none' };

type WireTas = TengriFile['track']['tas'];

const buildFile = (hash: number, tas: WireTas): TengriFile => ({
  version: 5,
  metadata: {
    takeoff_offset: 0,
    landing_offset: 0,
    takeoff_lat: 0,
    takeoff_lon: 0,
    landing_lat: 0,
    landing_lon: 0,
  },
  track: {
    start_time: START_TIME,
    interval: INTERVAL,
    track: EnumVariantValue('Dual', { fixes: FIXES, coords: COORDS }),
    time_fixes: TIME_FIXES,
    tas,
    hash,
  },
});

const noTasWire = EnumVariantValue('None', undefined) as WireTas;
const tasWire = (fixes: TasFix[], deltas: number[]): WireTas =>
  EnumVariantValue('Tas', { fixes, deltas }) as WireTas;

const validHashNoTas = computeCompactHash(
  START_TIME,
  INTERVAL,
  { dual: true, fixes: FIXES, coords: COORDS },
  TIME_FIXES,
  NO_TAS_UNPACKED,
);

describe('decodeTrack', () => {
  it('reconstructs an SoA track from a tiny dual-format file', async () => {
    const track = await decodeTrack(buildFile(validHashNoTas, noTasWire));

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

  it('exposes track.tas = null when the source had no TAS column', async () => {
    const track = await decodeTrack(buildFile(validHashNoTas, noTasWire));
    expect(track.tas).toBeNull();
  });

  it('reconstructs the TAS channel via fixes + i8 deltas', async () => {
    const tasFixes: TasFix[] = [{ idx: 0, tas: 50 }];
    const tasDeltas = [3, 0, -1];
    const tas: UnpackedTas = {
      kind: 'tas',
      fixes: tasFixes,
      deltas: tasDeltas,
    };
    const hash = computeCompactHash(
      START_TIME,
      INTERVAL,
      { dual: true, fixes: FIXES, coords: COORDS },
      TIME_FIXES,
      tas,
    );

    const track = await decodeTrack(
      buildFile(hash, tasWire(tasFixes, tasDeltas)),
    );

    expect(track.tas).not.toBeNull();
    expect(Array.from(track.tas!)).toEqual([50, 53, 53, 52]);
  });

  it('honours absolute TAS fix overrides past idx=0', async () => {
    const tasFixes: TasFix[] = [
      { idx: 0, tas: 50 },
      { idx: 2, tas: 200 },
    ];
    const tasDeltas = [3, -5];
    const tas: UnpackedTas = {
      kind: 'tas',
      fixes: tasFixes,
      deltas: tasDeltas,
    };
    const hash = computeCompactHash(
      START_TIME,
      INTERVAL,
      { dual: true, fixes: FIXES, coords: COORDS },
      TIME_FIXES,
      tas,
    );

    const track = await decodeTrack(
      buildFile(hash, tasWire(tasFixes, tasDeltas)),
    );

    expect(Array.from(track.tas!)).toEqual([50, 53, 200, 195]);
  });

  it('throws TrackDecodeError when the embedded hash does not match', async () => {
    await expect(
      decodeTrack(buildFile(0xdeadbeef, noTasWire)),
    ).rejects.toBeInstanceOf(TrackDecodeError);
  });

  it('throws when the TAS channel length does not match the point count', async () => {
    const tasFixes: TasFix[] = [{ idx: 0, tas: 50 }];
    // 1 fix + 2 deltas = 3, but track has 4 points.
    const tasDeltas = [1, 1];
    const tas: UnpackedTas = {
      kind: 'tas',
      fixes: tasFixes,
      deltas: tasDeltas,
    };
    const hash = computeCompactHash(
      START_TIME,
      INTERVAL,
      { dual: true, fixes: FIXES, coords: COORDS },
      TIME_FIXES,
      tas,
    );

    await expect(
      decodeTrack(buildFile(hash, tasWire(tasFixes, tasDeltas))),
    ).rejects.toBeInstanceOf(TrackDecodeError);
  });
});
