import type { FixDual, TengriFile } from '../../api/tracks.io';
import { runWithRaf } from '../../utils/runWithRaf';
import { TrackDecodeError, type Track } from '../types';
import { computeCompactHash } from './hash';
import {
  decodeStep,
  type DecodeCtx,
  type DecodeState,
  type TasCtx,
} from './step';
import { unpackBody, type UnpackedTas } from './unpackBody';

export interface DecodeOptions {
  signal?: AbortSignal;
  onProgress?: (done: number, total: number) => void;
  frameBudgetMs?: number;
}

/**
 * Decode time-sliced across animation frames. Pass `signal` to cancel.
 */
export async function decodeTrack(
  file: TengriFile,
  { signal, onProgress, frameBudgetMs }: DecodeOptions = {},
): Promise<Track> {
  const prep = prepare(file);
  const { ctx, state } = prep;

  await runWithRaf({
    step: () => decodeStep(state, ctx),
    isDone: () => state.i >= ctx.length,
    signal,
    frameBudgetMs,
    onFrame: onProgress ? () => onProgress(state.i, ctx.length) : undefined,
  });

  return finish(prep);
}

interface Prepared {
  ctx: DecodeCtx;
  state: DecodeState;
  startTime: number;
}

function prepare(file: TengriFile): Prepared {
  const { startTime, interval, body, timeFixes, tas } = unpackBody(file);
  const { dual, fixes, coords } = body;
  const length = fixes.length + coords.length;

  if (length === 0) {
    throw new TrackDecodeError('track is empty');
  }

  if (fixes.length === 0 || fixes[0]!.idx !== 0) {
    throw new TrackDecodeError('track is missing the initial fix at idx=0');
  }

  const expectedHash = file.track.hash;
  const actualHash = computeCompactHash(
    startTime,
    interval,
    body,
    timeFixes,
    tas,
  );
  if (actualHash !== expectedHash) {
    throw new TrackDecodeError(
      `hash mismatch: expected 0x${expectedHash.toString(16).padStart(8, '0')}, got 0x${actualHash.toString(16).padStart(8, '0')}`,
    );
  }

  const t = new Uint32Array(length);
  const lat = new Int32Array(length);
  const lng = new Int32Array(length);
  const alt = new Int32Array(length);
  const baroAlt = dual ? new Int32Array(length) : null;
  const { tasCtx, sTas } = prepareTas(tas, length);

  const f0 = fixes[0]!;
  const state: DecodeState = {
    i: 0,
    fixCur: 1,
    coordCur: 0,
    timeCur: 0,
    anchorIdx: 0,
    anchorTime: startTime,
    sLat: f0.lat,
    sLng: f0.lon,
    sAlt: f0.geo_alt,
    sBaroAlt: dual ? (f0 as FixDual).pressure_alt : 0,
    tasFixCur: tasCtx ? 1 : 0,
    tasDeltaCur: 0,
    sTas,
  };

  const ctx: DecodeCtx = {
    fixes,
    coords,
    timeFixes,
    interval,
    dual,
    length,
    t,
    lat,
    lng,
    alt,
    baroAlt,
    tas: tasCtx,
  };

  return { ctx, state, startTime };
}

function finish({ ctx, startTime }: Prepared): Track {
  return {
    startTime,
    t: ctx.t,
    lat: ctx.lat,
    lng: ctx.lng,
    alt: ctx.alt,
    baroAlt: ctx.baroAlt,
    tas: ctx.tas?.out ?? null,
  };
}

/**
 * Validate the TAS channel shape and produce a `TasCtx` plus the initial
 * running state. `TasBody::None` returns `tasCtx: null` and a sentinel
 * `sTas: 0` that the step loop never observes (gated by `if (c.tas)`).
 */
function prepareTas(
  tas: UnpackedTas,
  length: number,
): { tasCtx: TasCtx | null; sTas: number } {
  if (tas.kind === 'none') {
    return { tasCtx: null, sTas: 0 };
  }
  const { fixes, deltas } = tas;
  if (fixes.length === 0 || fixes[0]!.idx !== 0) {
    throw new TrackDecodeError(
      'TAS channel is missing the initial fix at idx=0',
    );
  }

  if (fixes.length + deltas.length !== length) {
    throw new TrackDecodeError(
      `TAS channel length mismatch: ${fixes.length} fixes + ${deltas.length} deltas != ${length} points`,
    );
  }

  for (let i = 1; i < fixes.length; i++) {
    if (fixes[i]!.idx <= fixes[i - 1]!.idx) {
      throw new TrackDecodeError(
        `TAS channel has non-increasing fix indices: ${fixes[i - 1]!.idx} -> ${fixes[i]!.idx}`,
      );
    }
  }

  const out = new Uint16Array(length);
  return { tasCtx: { fixes, deltas, out }, sTas: fixes[0]!.tas };
}
