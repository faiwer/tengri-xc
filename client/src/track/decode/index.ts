import type { FixDual, TengriFile } from '../../api/tracks.io';
import { runWithRaf } from '../../utils/runWithRaf';
import { TrackDecodeError, type Track } from '../types';
import { decodeStep, type DecodeCtx, type DecodeState } from './step';
import { unpackBody } from './unpackBody';

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
  const { startTime, interval, body, timeFixes } = unpackBody(file);
  const { dual, fixes, coords } = body;
  const length = fixes.length + coords.length;

  if (length === 0) {
    throw new TrackDecodeError('track is empty');
  }

  if (fixes.length === 0 || fixes[0]!.idx !== 0) {
    throw new TrackDecodeError('track is missing the initial fix at idx=0');
  }

  const t = new Uint32Array(length);
  const lat = new Int32Array(length);
  const lng = new Int32Array(length);
  const alt = new Int32Array(length);
  const baroAlt = dual ? new Int32Array(length) : null;

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
  };
}
